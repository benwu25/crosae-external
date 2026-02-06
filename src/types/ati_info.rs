/* Because we are invoking the compiler multiple times, we need some 
 * way of relaying information between the multiple compilations. This file
 * defines some structs which can be used for just that.
 * 
 * FunctionBoundaries is used to relay information from the first pass, which 
 * discovers what functions we are going to be instrumenting and where we are 
 * making calls to untracked functions. 
 * 
 * FunctionBoundaries is then used to during the second compilation to only
 * instrument specific functions, during which FunctionSignatures is constructed.
 * FunctionSignatures is used to record the updated data types used in function 
 * inputs and outputs, as well as the function name and parameter names.
 * FunctionSignatures is then consumed by the stub creation process, to add in 
 * the correct stubs responsible for managing sites.
*/

use std::collections::{HashMap, HashSet};

use rustc_ast::{Param, ast};
use rustc_hir::def_id::DefId;
use rustc_middle as mir;
use rustc_span::{Ident, Span};

use crate::common;

/// Contains all information that is going to be passed between the
/// first and second compilation rounds. Populated by invoking the
/// compiler, using the GatherAtiInfo callbacks.
#[derive(Debug)]
pub struct FunctionBoundaries {
    /// which user-defined functions are instrumented across the entire project
    tracked_fn_def_ids: HashSet<DefId>,
    tracked_fn_idents: HashSet<Ident>,

    /// places where a non-tracked function is called
    /// mapped to a string representation of the return type at that point.
    // FIXME: I'm not convinced that a string here is the best thing to store
    // but until I see an actual use for that, idc. Could be the mir::ty::Ty.
    untracked_fn_calls: HashMap<Span, String>,
}

impl FunctionBoundaries {
    pub fn new() -> Self {
        Self {
            tracked_fn_def_ids: HashSet::new(),
            tracked_fn_idents: HashSet::new(),
            untracked_fn_calls: HashMap::new(),
        }
    }

    ///////
    // Learn info

    /// register that a function with `ident` and `def_id` should 
    /// later instrumented.
    pub fn observe_tracked_fn(&mut self, ident: &Ident, def_id: DefId) {
        self.tracked_fn_idents.insert(ident.clone());
        self.tracked_fn_def_ids.insert(def_id);
    }

    /// register that a function call was made to an untracked funtion at
    /// `loc`, which returned a value of type `ty`.
    pub fn observe_untracked_fn_call<'a>(&mut self, loc: Span, ty: mir::ty::Ty<'a>) {
        self.untracked_fn_calls.insert(loc, ty.to_string());
    }

    ///////
    // Use info

    /// returns true if this identifier represent a tracked function.
    pub fn is_fn_ident_tracked(&self, ident: &Ident) -> bool {
        self.tracked_fn_idents.contains(ident)
    }

    /// returns true if this def_id represents a tracked function.
    pub fn is_fn_def_id_tracked(&self, def_id: &DefId) -> bool {
        self.tracked_fn_def_ids.contains(def_id)
    }

    /// fetches the original type returned from an untracked function call,
    /// if one exists at that location.
    pub fn get_untracked_fn_call_ret_ty(&self, location: &Span) -> Option<&String> {
        self.untracked_fn_calls.get(location)
    }
}

/// This struct is responsible for packaging together the new function signatures
/// of functions that were modified, for which function stubs need to be created.
/// Each stub requires knowledge of the function name, param names + types, and the
/// return type, all of which is encoded in the `tracked` map.
#[derive(Debug)]
pub struct FunctionSignatures {
    tracked: HashMap<String, (Vec<(String, bool, String)>, Option<String>)>,
}
impl FunctionSignatures {
    /// Constructor
    pub fn new() -> Self {
        Self {
            tracked: HashMap::new(),
        }
    }

    /// Gets the string name of the parameter that this ast Param refers to
    fn get_param_name(&self, param: &&mut Param) -> String {
        match param.pat.kind {
            rustc_ast::PatKind::Ident(_, ident, _) => ident.as_str().to_string(),
            _ => {
                panic!("shouldn't happen?")
            }
        }
    }

    /// Observes a new function signature, with the given name, inputs, and output
    pub fn register_fn_sig(
        &mut self,
        name: &str,
        inputs: Vec<&mut Param>,
        output: Option<&ast::Ty>,
    ) {
        let inputs = inputs
            .iter()
            .map(|param| {
                (
                    self.get_param_name(param),
                    common::is_type_tupled(&param.ty),
                    common::get_type_string(&param.ty),
                )
            })
            .collect::<Vec<_>>();
        let output = output.map(|ty| common::get_type_string(ty));

        self.tracked.insert(name.to_string(), (inputs, output));
    }

    /// Returns an iterator over all observed function signatures.
    // FIXME: i think it could be slightly cleaner to have a
    // create_stub(fn_name) type thing defined here, that returns the full stub.
    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (&String, &(Vec<(String, bool, String)>, Option<String>))> {
        self.tracked.iter()
    }
}

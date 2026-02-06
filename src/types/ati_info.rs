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
    // which user-defined functions are instrumented across the entire project
    tracked_fn_def_ids: HashSet<DefId>,
    tracked_fn_idents: HashSet<Ident>,

    // places where a non-tracked function is called
    // mapped to a string representation of the return type at that point.
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
    /// Learn info
    pub fn observe_tracked_fn(&mut self, ident: &Ident, def_id: DefId) {
        self.tracked_fn_idents.insert(ident.clone());
        self.tracked_fn_def_ids.insert(def_id);
    }

    pub fn observe_untracked_fn_call<'a>(&mut self, loc: Span, ty: mir::ty::Ty<'a>) {
        self.untracked_fn_calls.insert(loc, ty.to_string());
    }

    ///////
    /// Use info
    pub fn is_fn_ident_tracked(&self, ident: &Ident) -> bool {
        self.tracked_fn_idents.contains(ident)
    }

    pub fn is_fn_def_id_tracked(&self, def_id: &DefId) -> bool {
        self.tracked_fn_def_ids.contains(def_id)
    }

    pub fn get_tracked_fn_idents(&self) -> &HashSet<Ident> {
        &self.tracked_fn_idents
    }

    pub fn is_span_an_untracked_func_call(&self, loc: &Span) -> bool {
        self.untracked_fn_calls.contains_key(loc)
    }

    pub fn get_untracked_fn_call_ret_ty(&self, location: &Span) -> Option<&String> {
        self.untracked_fn_calls.get(location)
    }
}

#[derive(Debug)]
pub struct FunctionSignatures {
    tracked: HashMap<String, (Vec<(String, bool, String)>, Option<String>)>,
}
impl FunctionSignatures {
    pub fn new() -> Self {
        Self {
            tracked: HashMap::new(),
        }
    }

    fn get_param_name(&self, param: &&mut Param) -> String {
        match param.pat.kind {
            rustc_ast::PatKind::Ident(_, ident, _) => ident.as_str().to_string(),
            _ => {
                panic!("shouldn't happen?")
            }
        }
    }

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

    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (&String, &(Vec<(String, bool, String)>, Option<String>))> {
        self.tracked.iter()
    }
}

/* Defines the visitor which edits all type signatures and definitions to 
 * wrap primitive types T into TaggedValue<T> (defined in ati.rs). 
 * After this pass, all declared types should be in a form which allows
 * unique tags to be carried alongside values.
*/
use std::collections::HashMap;

use rustc_ast as ast;
use rustc_ast::mut_visit::MutVisitor;
use rustc_span::{DUMMY_SP, Ident};

use crate::instrumentation::common::{self, FnInfo};

// FIXME: this deserves a better name.
// it does more than functions after all
pub struct UpdateFnDeclsVisitor {
    modified_functions: HashMap<String, FnInfo>,
}

impl MutVisitor for UpdateFnDeclsVisitor {
    /// Converts all function signatures and top level type definitions (structs)
    /// to thier tagged variants. Specifically modifies all parameter types to
    /// be TaggedValues if necessary, alongside returns.
    fn visit_item(&mut self, item: &mut ast::Item) {
        match item.kind {
            ast::ItemKind::Fn(box ast::Fn {
                ref mut ident,
                sig: ast::FnSig { ref mut decl, .. },
                ..
            }) => {
                let mut params = Vec::new();
                // go through parameters of function...
                for param in &mut decl.inputs {
                    // ... and tuple any instances of basic types
                    self.recursively_tuple_type(&mut param.ty);
                    params.push(Box::new(param.clone()));
                }

                if let ast::FnRetTy::Ty(ref mut return_type) = decl.output {
                    // do the same to the return type, if one exists
                    self.recursively_tuple_type(return_type);
                }

                // store required info so that we can later make a stub version of this later
                self.modified_functions.insert(
                    ident.as_str().into(),
                    FnInfo {
                        params,
                        return_ty: Box::new(decl.output.clone()),
                    },
                );

                // rename the function to be *_unstubbed so the stub can call it
                let old_ident = ident.as_str();
                *ident = Ident::from_str(&format!("{old_ident}_unstubbed"));
            }

            // Tags all values in struct that can be tupled
            ast::ItemKind::Struct(_, _, ast::VariantData::Struct { ref mut fields, .. }) => {
                for field_def in fields {
                    self.recursively_tuple_type(&mut field_def.ty);
                }
            }

            // TODO: method defs in impl blocks?
            _ => {}
        }
    }
}

impl UpdateFnDeclsVisitor {
    /// Constructor
    pub fn new() -> Self {
        Self {
            modified_functions: HashMap::new(),
        }
    }

    /// Extract the information regarding functions that this visitor has 
    /// discovered and considered tracked
    pub fn get_modified_funcs(&self) -> &HashMap<String, FnInfo> {
        &self.modified_functions
    }

    /// Directly modifies a type T into a TaggedValue<T> in place.
    fn tuple_type(&self, old_type: &mut ast::Ty) {
        old_type.kind = ast::TyKind::Path(
            None,
            ast::Path {
                segments: [ast::PathSegment {
                    ident: Ident::from_str("TaggedValue"),
                    id: ast::DUMMY_NODE_ID,
                    args: Some(Box::new(ast::AngleBracketed(ast::AngleBracketedArgs {
                        span: DUMMY_SP,
                        args: [ast::AngleBracketedArg::Arg(ast::GenericArg::Type(
                            Box::new(old_type.clone()),
                        ))]
                        .into(),
                    }))),
                }]
                .into(),
                span: DUMMY_SP,
                tokens: None,
            },
        );
    }

    /// Searches through type `ty` to find and tuple all primitive types 
    /// that should be tupled. Modifies the type in place.
    /// Strips off references (both & and &mut), acting on the actual referenced-types.
    fn recursively_tuple_type<'a>(&self, ty: &'a mut ast::Ty) {
        let peeled_type = peel_refs(ty);

        if common::can_type_be_tupled(peeled_type) {
            self.tuple_type(peeled_type);
            return;
        }

        if let ast::TyKind::Path(_, path) = &mut peeled_type.kind {
            for segment in path.segments.iter_mut() {
                if let Some(box ast::GenericArgs::AngleBracketed(ast::AngleBracketedArgs{
                    args,
                    ..
                })) = &mut segment.args {
                    for arg in args.iter_mut() {
                        if let ast::AngleBracketedArg::Arg(ast::GenericArg::Type(ty)) = arg {
                            self.recursively_tuple_type(ty);
                        } else {
                            // TODO: Lifetimes (also above in type peeling?)
                            todo!();
                        }
                    }
                }
            }
        }
    }
}

// TODO: this is equiv to Ty::peel_refs but just mutable rather than shared borrows
// there has to be a better way!
fn peel_refs(ty: &mut ast::Ty) -> &mut ast::Ty {
    let mut final_ty = ty;
    while let ast::TyKind::Ref(_, ast::MutTy { ref mut ty, .. }) | ast::TyKind::Ptr(ast::MutTy { ref mut ty, .. }) = final_ty.kind
    {
        final_ty = &mut **ty;
    }

    final_ty
}

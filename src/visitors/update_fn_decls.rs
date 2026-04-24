/* Defines the visitor which edits all type signatures and definitions to
 * wrap primitive types T into TaggedValue<T> (defined in ati.rs).
 * After this pass, all declared types should be in a form which allows
 * unique tags to be carried alongside values.
*/
use rustc_ast as ast;
use rustc_ast::mut_visit::MutVisitor;
use rustc_span::{DUMMY_SP, Ident};

use crate::common;
use crate::types::ati_info::{FunctionBoundaries, FunctionSignatures};

pub struct UpdateFnDeclsVisitor<'a> {
    fbs: &'a FunctionBoundaries,
    fn_sigs: Option<FunctionSignatures>,
}

impl<'a> MutVisitor for UpdateFnDeclsVisitor<'a> {
    /// Converts all function signatures and top level type definitions (structs)
    /// to thier tagged variants. Specifically modifies all parameter types to
    /// be TaggedValues if necessary, alongside returns.
    fn visit_item(&mut self, item: &mut ast::Item) {
        match &mut item.kind {
            // Tags all input and return types that can be tupled in fn sigs
            ast::ItemKind::Fn(box ast::Fn {
                ident,
                sig: ast::FnSig { decl, .. },
                ..
            }) => {
                if !self.fbs.is_fn_ident_tracked(ident) {
                    // we have previously decided that this function is not tracked and shouldn't be instrumented
                    return;
                }

                // adds a TaggedValue<*> around all taggable types, recursively
                for param in &mut decl.inputs {
                    self.recursively_tuple_type(&mut param.ty);
                }

                // we know this function is tracked, at some point, it will need a stub made
                // which requires knowledge of it's name, inputs, and outputs. Record all that info
                let orig_ident = ident.as_str();
                if let ast::FnRetTy::Ty(return_type) = &mut decl.output {
                    // do the recursive wrapping to the return type if one exists
                    self.recursively_tuple_type(return_type);
                    self.fn_sigs.as_mut().unwrap().register_fn_sig(
                        &orig_ident,
                        decl.inputs.iter().collect(),
                        Some(return_type),
                    );
                } else {
                    self.fn_sigs.as_mut().unwrap().register_fn_sig(
                        &orig_ident,
                        decl.inputs.iter().collect(),
                        None,
                    );
                }

                // rename the function to be *_unstubbed so the stub can call it
                *ident = Ident::from_str(&format!("{orig_ident}_unstubbed"));
            }

            // Tags all values in struct defs that can be tupled
            // FIXME: generics????
            ast::ItemKind::Struct(ident, generics, ast::VariantData::Struct { fields, .. }) => {
                for field_def in fields.iter_mut() {
                    self.recursively_tuple_type(&mut field_def.ty);
                }

                self.fn_sigs
                    .as_mut()
                    .unwrap()
                    .register_struct_def(ident.as_str(), fields.iter().collect());
            }

            // TODO: method defs in impl blocks?
            _ => {}
        }
    }
}

impl<'a> UpdateFnDeclsVisitor<'a> {
    /// Constructor
    pub fn new(fbs: &'a FunctionBoundaries) -> Self {
        Self {
            fbs,
            fn_sigs: Some(FunctionSignatures::new()),
        }
    }

    /// Pulls out all information about function signatures that this visitor
    /// modified. Panics if invoked before the pass is performed.
    pub fn get_new_fn_signatures(&mut self) -> FunctionSignatures {
        self.fn_sigs.take().expect("FnSigs was already taken!")
    }

    /// Directly modifies a type T into a TaggedValue<T> in place,
    /// assumes that T is known to be tupleable.
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
    fn recursively_tuple_type<'b>(&self, ty: &'b mut ast::Ty) {
        let peeled_type = common::peel_refs(ty);

        // base case, the type can just be tupled and no recursion is necessary
        if common::can_type_be_tupled(peeled_type) {
            self.tuple_type(peeled_type);
            return;
        }

        match &mut peeled_type.kind {
            rustc_ast::TyKind::Slice(ty) | rustc_ast::TyKind::Array(ty, _) => {
                self.recursively_tuple_type(ty);
            }

            rustc_ast::TyKind::Ptr(ast::MutTy { box ty, .. })
            | rustc_ast::TyKind::Ref(_, ast::MutTy { box ty, .. }) => {
                self.recursively_tuple_type(ty);
            }

            rustc_ast::TyKind::FnPtr(box ast::FnPtrTy {
                generic_params,
                decl: box ast::FnDecl { inputs, output },
                ..
            }) => {
                // tuple all generic types for this function pointer
                for generic in generic_params {
                    match &mut generic.kind {
                        rustc_ast::GenericParamKind::Type { default } => {
                            if let Some(ty) = default {
                                self.recursively_tuple_type(ty);
                            }
                        }
                        rustc_ast::GenericParamKind::Const { ty, .. } => {
                            self.recursively_tuple_type(ty);
                        }
                        rustc_ast::GenericParamKind::Lifetime => {}
                    }
                }

                // tuple all param input types
                for input in inputs {
                    self.recursively_tuple_type(&mut input.ty)
                }

                // tuple output type, if one exists
                if let ast::FnRetTy::Ty(box ty) = output {
                    self.recursively_tuple_type(ty);
                }
            }

            rustc_ast::TyKind::Tup(tys) => {
                for ty in tys {
                    self.recursively_tuple_type(ty);
                }
            }

            rustc_ast::TyKind::Path(_, ast::Path { segments, .. }) => {
                // traverse path::to::func() by segment, if any generics exist on any of the paths,
                // tuple those generic types
                for segment in segments.iter_mut() {
                    if let Some(box arg) = &mut segment.args {
                        match arg {
                            rustc_ast::GenericArgs::AngleBracketed(ast::AngleBracketedArgs {
                                args,
                                ..
                            }) => {
                                for arg in args.iter_mut() {
                                    match arg {
                                        rustc_ast::AngleBracketedArg::Arg(generic_arg) => {
                                            match generic_arg {
                                                rustc_ast::GenericArg::Type(ty) => {
                                                    self.recursively_tuple_type(ty);
                                                }
                                                rustc_ast::GenericArg::Const(_)
                                                | rustc_ast::GenericArg::Lifetime(_) => {}
                                            }
                                        }
                                        rustc_ast::AngleBracketedArg::Constraint(_) => {
                                            todo!("Constraint is a trait?")
                                        }
                                    }
                                }
                            }
                            rustc_ast::GenericArgs::Parenthesized(ast::ParenthesizedArgs {
                                inputs,
                                output,
                                ..
                            }) => {
                                for input in inputs {
                                    self.recursively_tuple_type(input)
                                }

                                if let ast::FnRetTy::Ty(box ty) = output {
                                    self.recursively_tuple_type(ty);
                                }
                            }
                            rustc_ast::GenericArgs::ParenthesizedElided(span) => {
                                panic!("this panic is probably fine to remove")
                            }
                        }
                    }
                }
            }

            // maybe impl later
            rustc_ast::TyKind::PinnedRef(_, _) => todo!(),
            rustc_ast::TyKind::Pat(_, _) => todo!(),

            // probably left untouched
            rustc_ast::TyKind::Infer => panic!(),
            rustc_ast::TyKind::TraitObject(_, _) => panic!(),
            rustc_ast::TyKind::Paren(_) => panic!(),
            rustc_ast::TyKind::UnsafeBinder(_) => panic!(),
            rustc_ast::TyKind::Never => panic!(),
            rustc_ast::TyKind::ImplTrait(_, _) => panic!(),
            rustc_ast::TyKind::ImplicitSelf => panic!(),
            rustc_ast::TyKind::MacCall(_) => panic!(),
            rustc_ast::TyKind::CVarArgs => panic!(),
            rustc_ast::TyKind::Dummy => panic!(),
            rustc_ast::TyKind::Err(_) => panic!(),
            _ => todo!()
        };
    }
}

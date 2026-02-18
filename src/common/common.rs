/* Provides helper functions that are used throughout this entire project.
 * Namely, this includes determining the set of types that are considered
 * able to be tagged, as well as moving between different representations of types.
*/
use rustc_ast::token::{Lit, LitKind};
use rustc_ast::{self as ast};
use rustc_parse::lexer::StripTokens;
use rustc_parse::new_parser_from_source_str;
use rustc_parse::parser::ForceCollect;
use rustc_session::parse::ParseSess;
use rustc_span::{FileName, RealFileName, sym};

use std::path::Path;

/// Determines whether or not the passed in literal can be converted
/// into a TaggedValue. Modify the below list to enable/disable tupling literals.
pub fn can_literal_be_tupled(lit: &Lit) -> bool {
    match lit.kind {
        LitKind::Integer | LitKind::Float => true,
        _ => false,
    }
}

// TODO: this is equiv to Ty::peel_refs but just mutable rather than shared borrows
// there has to be a better way!
pub fn peel_refs(ty: &mut ast::Ty) -> &mut ast::Ty {
    let mut final_ty = ty;
    while let ast::TyKind::Ref(_, ast::MutTy { ref mut ty, .. })
    | ast::TyKind::Ptr(ast::MutTy { ref mut ty, .. }) = final_ty.kind
    {
        final_ty = &mut **ty;
    }

    final_ty
}

/// Determines if this type string (like "u32", or "Vec<u32>") can be tupled.
/// Currently does this naively, only numeric primitives are allowed to be tupled.
// TODO: refine this function, it would be nice to not use strings
// for this use case at all, and instead mir::Tys. mir::Tys are interned though,
// and should be cleaned by this point. Maybe its possible to do mir::Ty -> ast::Ty.
// It's also kind of gross that this function is different from `can_type_be_tupled`
pub fn can_type_string_be_tupled(ty_str: &str) -> bool {
    [
        "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "f16", "f32", "f64",
        "f128",
    ]
    .iter()
    .any(|prefix| ty_str.starts_with(prefix))
}

/// Determines whether or not the passed in ast type can be converted into
/// a TaggedValue. Modify the below list to add/remove tupled types.
pub fn can_type_be_tupled(ty: &ast::Ty) -> bool {
    // this function is very similar to ast::TyKind::maybe_scalar
    // but I'm leaving it here so that we have more control over it
    let ty = ty.peel_refs(); // ignore & and &mut, we care about actual type
    let Some(ty_sym) = ty.kind.is_simple_path() else {
        return false; // unit type then, which idt we need to track at all
    };

    matches!(
        ty_sym,
        sym::i8
            | sym::i16
            | sym::i32
            | sym::i64
            | sym::i128
            | sym::u8
            | sym::u16
            | sym::u32
            | sym::u64
            | sym::u128
            | sym::f16
            | sym::f32
            | sym::f64
            | sym::f128
    )
}

/// Naively determines if the passed in ast type is wrapped in a TaggedValue
/// at the top level.
pub fn is_type_tupled(ty: &ast::Ty) -> bool {
    let ty = ty.peel_refs(); // ignore & and &mut, we care about actual type
    match &ty.kind {
        rustc_ast::TyKind::Path(_, ast::Path { segments, .. }) => {
            segments
                .iter()
                .last()
                .expect("Unable to find last struct ident in type")
                .ident
                .as_str()
                == "TaggedValue"
        }

        rustc_ast::TyKind::Array(ty, _) => is_type_tupled(ty),

        _ => false, // rustc_ast::TyKind::Slice(ty) => todo!(),
                    // rustc_ast::TyKind::Ptr(mut_ty) => todo!(),
                    // rustc_ast::TyKind::Ref(lifetime, mut_ty) => todo!(),
                    // rustc_ast::TyKind::PinnedRef(lifetime, mut_ty) => todo!(),
                    // rustc_ast::TyKind::FnPtr(fn_ptr_ty) => todo!(),
                    // rustc_ast::TyKind::UnsafeBinder(unsafe_binder_ty) => todo!(),
                    // rustc_ast::TyKind::Never => todo!(),
                    // rustc_ast::TyKind::TraitObject(generic_bounds, trait_object_syntax) => todo!(),
                    // rustc_ast::TyKind::ImplTrait(node_id, generic_bounds) => todo!(),
                    // rustc_ast::TyKind::Paren(ty) => todo!(),
                    // rustc_ast::TyKind::Infer => todo!(),
                    // rustc_ast::TyKind::ImplicitSelf => todo!(),
                    // rustc_ast::TyKind::MacCall(mac_call) => todo!(),
                    // rustc_ast::TyKind::CVarArgs => todo!(),
                    // rustc_ast::TyKind::Pat(ty, ty_pat) => todo!(),
                    // rustc_ast::TyKind::Dummy => todo!(),
                    // rustc_ast::TyKind::Err(error_guaranteed) => todo!(),
    }
}

/// Takes an ast lifetime and turns it into a regular "'name" string.
fn get_lifetime_string(lifetime: &ast::Lifetime) -> String {
    format!("'{}", lifetime.ident.to_string())
}

/// Converts an ast Ty into the full type string, recursively.
// NIT: i hate the way that I'm parsing strings here, feels like a lot of unnecessary format!s
// I also think there might be a way to go from Span -> underlying text repr. would be really nice here
pub fn get_type_string(ty_path: &ast::Ty) -> String {
    match &ty_path.kind {
        rustc_ast::TyKind::Slice(box ty) => format!("[{}]", get_type_string(ty)),
        rustc_ast::TyKind::Ref(lifetime, ast::MutTy {
            box ty,
            mutbl,
        }) => {
            let mut_str = mutbl.prefix_str();
            let lt_str = match lifetime {
                Some(lifetime) => format!("{} ", get_lifetime_string(lifetime)),
                None => "".to_string(),
            };
            let refed_type_str = get_type_string(ty);

            format!("&{lt_str}{mut_str}{refed_type_str}")
        },
        rustc_ast::TyKind::Tup(v) => {
            let types = v.iter().map(|box ty| {
                get_type_string(ty)
            }).collect::<Vec<_>>().join(", ");

            format!("({types})")
        },

        // idk what qself really does... ignoring for now
        rustc_ast::TyKind::Path(qself, path) => {
            path.segments.iter().map(|segment| {
                let ident_str = segment.ident.to_string();

                // these are the <Generic, Args> passed in to this segment
                let generics_str = if let Some(box generics) = &segment.args {
                    match generics {
                        ast::GenericArgs::AngleBracketed(ast::AngleBracketedArgs{
                            args,
                            ..
                        }) => {
                            // <'a, A, B, C>
                            let arg_list_string = args.iter().map(|arg| {
                                match arg {
                                    rustc_ast::AngleBracketedArg::Arg(generic_arg) => {
                                        match generic_arg {
                                            rustc_ast::GenericArg::Lifetime(lifetime) => get_lifetime_string(lifetime),
                                            rustc_ast::GenericArg::Type(box ty) => get_type_string(&ty),
                                            rustc_ast::GenericArg::Const(anon_const) => {
                                                // idt this is going to be helpful for us
                                                unimplemented!()
                                            },
                                        }
                                    },
                                    rustc_ast::AngleBracketedArg::Constraint(assoc_item_constraint) => {
                                        // ": Trait"
                                        // this also has to be done at some point
                                        unimplemented!();

                                    },
                                }
                            }).collect::<Vec<_>>().join(", ");

                            format!("<{arg_list_string}>")
                        },
                        ast::GenericArgs::Parenthesized(ast::ParenthesizedArgs {
                            inputs,
                            output,
                            ..
                        }) => {
                            // (A, B) -> C
                            let input_list_str = inputs.iter().map(|box input_ty| {
                                get_type_string(input_ty)
                            }).collect::<Vec<_>>().join(", ");

                            let output_str = match output {
                                rustc_ast::FnRetTy::Default(_) => "".into(),  // unit type return
                                rustc_ast::FnRetTy::Ty(box ty) => format!(" -> {}", get_type_string(ty)),
                            };

                            format!("({input_list_str}){output_str}")
                        },
                        ast::GenericArgs::ParenthesizedElided(span) => {
                            // (..)
                            // i've never even seen this before
                            unimplemented!()
                        },
                    }
                } else {
                    "".into()
                };

                format!("{ident_str}{generics_str}")
            }).collect::<Vec<_>>().join("::")
        },

        rustc_ast::TyKind::Array(box ty, ast::AnonConst {
            value: box ast::Expr {
                kind: ast::ExprKind::Lit(ast::token::Lit {
                    symbol,
                    ..
                }),
                ..
            },
            ..
        }) => {
            let inner = get_type_string(ty);
            let constant = symbol.as_str();

            let res = format!("[{inner}; {constant}]");
            // panic!("{res:?}");
            res
        },

        rustc_ast::TyKind::Array(box ty, ast::AnonConst {
            value,
            ..
        }) => {
            // panic!("Found array with non-literal size:\n{ty:#?}\n{value:#?}");
            "[ARRAY]".into()
        },

        // this should be impossible, for now error out
        rustc_ast::TyKind::ImplicitSelf |  // def necessary at some point
        rustc_ast::TyKind::MacCall(_) |
        rustc_ast::TyKind::CVarArgs |
        rustc_ast::TyKind::Pat(_, _) |
        rustc_ast::TyKind::Err(_) |
        rustc_ast::TyKind::Dummy |
        rustc_ast::TyKind::Paren(_) |
        rustc_ast::TyKind::TraitObject(_, _) | // prob necessary at some point
        rustc_ast::TyKind::ImplTrait(_, _) | // also this 
        rustc_ast::TyKind::Never |
        rustc_ast::TyKind::UnsafeBinder(_) |
        rustc_ast::TyKind::FnPtr(_) |
        rustc_ast::TyKind::PinnedRef(_, _) |
        rustc_ast::TyKind::Ptr(_) => {
            todo!("I still don't really know what to do with these types");
            // they are either weird to include for the current use case, or just won't be supported
        },

        // we are trying to get a well formed type string. 
        // encountering this means thats impossible
        rustc_ast::TyKind::Infer => panic!(),
    }
}

fn create_parser<'a>(
    psess: &'a ParseSess,
    contents: String,
    file_path: Option<&Path>,
) -> rustc_parse::parser::Parser<'a> {
    new_parser_from_source_str(
        psess,
        match file_path {
            Some(path) => FileName::Real(RealFileName::from_virtual_path(path)),
            None => FileName::anon_source_code(&contents),
        },
        contents,
        StripTokens::Nothing,
    )
    .unwrap()
}

/// Parses a string `contents` into a vector of ast::Items that can then be inserted into
/// any crate.
pub fn parse_items(
    psess: &ParseSess,
    contents: String,
    file_path: Option<&Path>,
) -> Vec<Box<ast::Item>> {
    let mut parser = create_parser(psess, contents, file_path);

    let mut res = Vec::new();
    loop {
        match parser.parse_item(ForceCollect::No) {
            Ok(Some(item)) => {
                res.push(item);
            }
            Ok(None) => break, // no more items
            Err(diag) => {
                diag.emit();
                panic!("Failed to parse item!");
            }
        }
    }

    res
}

pub fn parse_crate(psess: &ParseSess, contents: String, file_path: Option<&Path>) -> ast::Crate {
    let mut parser = create_parser(psess, contents, file_path);

    match parser.parse_crate_mod() {
        Ok(krate) => krate,
        Err(diag) => {
            diag.emit();
            panic!("Failed to parse crate!")
        }
    }
}

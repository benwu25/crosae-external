/* Provides helper functions that are used throughout this entire project.
 * Namely, this includes determining the set of types that are considered
 * able to be tagged, and carrying tracked function information from the
 * point where they are discovered by the visitor in params.rs, to the point
 * where stubs are created, in stubs.rs.
*/
use rustc_ast::{self as ast};
use rustc_ast::{Ty, token::{Lit, LitKind}};
use rustc_span::{sym};

// TODO: this whole file is due for a refactor.
// split out the things that help with types, split out the things 
// that help with functions, etc...

/// Returns true if the passed in node represents a type which 
/// is tupled at the top level (does not recursively search through generics)
fn is_type_tupled(ty: &Ty) -> bool {
    let ty = ty.peel_refs(); // ignore & and &mut, we care about actual type
    if let ast::TyKind::Path(_, ast::Path { ref segments, .. }) = ty.kind {
        segments[0].ident.as_str() == "TaggedValue"
    } else {
        false
    }
}

/// Determines whether or not the passed in literal can be converted
/// into a TaggedValue. Modify the below list to enable/disable tupling literals.
pub fn can_literal_be_tupled(lit: &Lit) -> bool {
    matches!(lit.kind, LitKind::Integer | LitKind::Float)
}

/// Determines whether or not the passed in type can be converted into
/// a TaggedValue. Modify the below list to add/remove tupled types.
pub fn can_type_be_tupled(ty: &Ty) -> bool {
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
            | sym::char
            | sym::bool
    )
}

fn get_lifetime_string(lifetime: &ast::Lifetime) -> String {
    format!("'{}", lifetime.ident.to_string())
}

/// Converts an ast Ty into the full type string,
// FIXME: i hate the way that I'm parsing strings here, feels like a lot of unnecessary format!s
// I also think there might be a way to go from Span -> underlying text repr. would be really nice here
fn get_type_string(ty_path: &ast::Ty) -> String {
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

                // these are the <Generic, Args>  passed in
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
        rustc_ast::TyKind::Array(_, _) |
        rustc_ast::TyKind::Ptr(_) => {
            todo!("I still don't really know what to do with these types");
            // they are either weird to include for the current use case, or just won't be supported
        },
        
        // we are trying to get a well formed type string. 
        // encountering this means thats impossible
        rustc_ast::TyKind::Infer => panic!(),
    }
}

/// Stores all information discovered by the UpdateFnDeclsVisitor about functions
/// that is necessary to create stub versions of all tracked functions.
#[derive(Debug)]
pub struct FnInfo {
    // FIXME: I honestly don't like the Boxes here, feels like simple
    // references will live long enough and avoid unnecessary clones
    pub params: Vec<Box<ast::Param>>,
    pub return_ty: Box<ast::FnRetTy>,
    // might want to add things like this to create full fledged stubs
    // visibility:
}

impl FnInfo {
    /// Creates string representations of the statements from ati.rs required 
    /// to bind all input parameters to the enter and exit sites.
    fn create_param_binds(&self, site_name: &str) -> String {
        self.params
            .iter()
            .filter(|param| is_type_tupled(&param.ty))
            .map(|param| {
                if let ast::PatKind::Ident(_, ref ident, _) = param.pat.kind {
                    let param_name = ident.as_str();
                    format!(
                        r#"
                        {site_name}.bind(stringify!({param_name}), {param_name}.clone());
                    "#
                    )
                    // TODO: is the above .clone() fine???
                    // TODO: what happens if a collection is passed across a function boundary?
                    // are all vars in the collection added to the site?
                } else {
                    unreachable!();
                }
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// Reads in self.params and constructs the string
    /// of parameter declarations to use for this function
    /// 
    /// In other words, returns the string described by <...>:
    /// `fn my_foo(< a: u32, b: f64 >);`
    fn create_param_decls(&self) -> String {
        // FIXME: probably combined this function with create_passed_params
        // println!("PARAMS: {:#?}", &self.params);
        self.params
            .iter()
            .map(|param| {
                if let ast::Param {
                    pat:
                        box ast::Pat {
                            kind: ast::PatKind::Ident(_, ident, _),
                            ..
                        },
                    ty,
                    ..
                } = &**param
                {
                    let param_name = ident.as_str();
                    let type_str = get_type_string(&ty);
                    format!(r#"{param_name}: {type_str}"#)
                } else {
                    unreachable!();
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Reads in self.params and constructs the string
    /// of parameters to pass into the *_unstubbed version 
    /// of the function.
    /// 
    /// In other words, returns the string described by <...>
    /// `let res = foo_unstubbed(< a, b >);``
    fn create_passed_params(&self) -> String {
        self.params
            .iter()
            .map(|param| {
                if let ast::Param {
                    pat:
                        box ast::Pat {
                            kind: ast::PatKind::Ident(_, ref ident, _),
                            ..
                        },
                    ..
                } = **param
                {
                    ident.as_str()
                } else {
                    unreachable!();
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Reads self.return_ty and converts the node
    /// into a regular type string. Return None if
    /// the return type is ().
    fn create_return_type(&self) -> Option<String> {
        if let box ast::FnRetTy::Ty(ty) = &self.return_ty
        {
            Some(get_type_string(ty))
        } else {
            None
        }
    }

    /// Creates function stubs that manage ::ENTER and ::EXIT information,
    /// and properly invoke the function described by self.
    pub fn create_fn_stub(&self, name: &str) -> String {
        if name == "main" {
            // TODO: environment stuff for main
            // this is kind of a silly stub for now...
            return format!(
                r#"
                fn main() {{
                    let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site(stringify!(main::ENTER));
                    ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                    main_unstubbed();

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site(stringify!(main::EXIT));
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);
                    ATI_ANALYSIS.lock().unwrap().report();
                }}
            "#
            );
        }

        let enter_param_binds = self.create_param_binds("site_enter");
        let exit_param_binds = self.create_param_binds("site_exit");
        let param_decls = self.create_param_decls();
        let params_passed = self.create_passed_params();
        let ret_ty = self.create_return_type();

        // TODO: do we want to add the params to site_exit before or after the function executes?
        // as in, do we do the site_exit stuff before *_unstubbed, or after? does it matter?
        if let Some(ret_ty) = ret_ty {
            // with a return value
            format!(
                r#"
                fn {name}({param_decls}) -> {ret_ty} {{
                    let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site(stringify!({name}::ENTER));
                    {enter_param_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                    let res = {name}_unstubbed({params_passed});

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site(stringify!({name}::EXIT));
                    {exit_param_binds}
                    site_exit.bind(stringify!(RET), res);
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);
                    return res;
                }}
            "#
            )
        } else {
            // without a return value
            format!(
                r#"
                fn {name}({param_decls}) {{
                    let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site(stringify!({name}::ENTER));
                    {enter_param_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                    {name}_unstubbed({params_passed});

                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site(stringify!({name}::EXIT));
                    {exit_param_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);
                    ATI_ANALYSIS.lock().unwrap().report();
                }}
            "#
            )
        }
    }
}

// fun fact, you can pull a lot more info off of the item node:
// i.e. skip test functions.
// for attr in attrs {
//     if let ast::AttrKind::Normal(normal_attr) = &attr.kind {
//         let path_str = normal_attr
//             .item
//             .path
//             .segments
//             .iter()
//             .map(|seg| seg.ident.as_str())
//             .collect::<Vec<_>>()
//             .join("::");
//         if path_str == "test" || path_str == "cfg" {
//             return true;
//         }
//     }
// }

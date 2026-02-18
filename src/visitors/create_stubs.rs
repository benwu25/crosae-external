/* Creates function stubs for each tracked function that was discovered.
 * Each stub sets up enter and exit sites before invoking the actual function.
 * Any formals are registered to both enter and exit sites, the return value
 * is also registered to the exit site, under the name "RET".
*/
use rustc_ast as ast;

use rustc_session::parse::ParseSess;

use crate::common;
use crate::types::ati_info::FunctionSignatures;

// /// Creates bind statements for all formals to a site with name `site_name`.
// fn create_site_binds<'a>(
//     site_name: &str,
//     inputs: &Vec<(String, TrackedParamType)>,
//     fn_sigs: &FunctionSignatures,
// ) -> String {
//     inputs
//         .iter()
//         .filter(|(_, ptype)| !matches!(ptype, TrackedParamType::Untracked(_)))
//         .map(|(name, ptype)| {
//             match ptype {
//                 TrackedParamType::Regular(_) => {
//                     format!(
//                         r#"
//                         {site_name}.bind("{name}", {name});
//                     "#
//                     )
//                 }
//                 TrackedParamType::Reference(_, is_tracked) => {
//                     if *is_tracked {
//                         format!(
//                             r#"
//                             {site_name}.bind("&{name}", {name});
//                             "#
//                         )
//                     } else {
//                         "".into()
//                     }
//                 } // something special?
//                 TrackedParamType::FixedLengthArray(_, size) => {
//                     // fixme: change this later to be just one element of the array
//                     format!(
//                         r#"
//                         for i in 0..{size} {{
//                             {site_name}.bind(&format!("{name}[{{}}]", i), {name}[i]);
//                         }}
//                     "#
//                     )
//                 }
//                 // this needs to be recursive to handle structs which contain structs
//                 TrackedParamType::Struct(_, tail) => {
//                     dbg!(&tail);
//                     match fn_sigs.get_struct_info(&tail) {
//                         Some(v) => {
//                             v.iter()
//                                 .filter(|(_, field_ty)| {
//                                     match field_ty {
//                                         TrackedParamType::Regular(_) |
//                                         TrackedParamType::Reference(_, _) |
//                                         TrackedParamType::FixedLengthArray(_, _) |
//                                         TrackedParamType::Struct(_, _) => true,

//                                         TrackedParamType::Untracked(_) => false,
//                                     }
//                                 }).map(|(field, field_ty)| {
//                                     match field_ty {
//                                         TrackedParamType::Regular(_) => {
//                                             format!(r#"{site_name}.bind("{name}.{field}", {name}.{field});"#)
//                                         },
//                                         TrackedParamType::Reference(_, is_tracked) => {
//                                             if *is_tracked {
//                                                 format!(r#"{site_name}.bind("&{name}.{field}", {name}.{field});"#)
//                                             } else {
//                                                 "".into()
//                                             }
//                                         },
//                                         TrackedParamType::FixedLengthArray(_, _) => todo!(),
//                                         TrackedParamType::Untracked(_) => todo!(),

//                                         TrackedParamType::Struct(_, _) => unreachable!(),
//                                     }
//                                 }).collect::<Vec<_>>().join("\n");

//                             todo!();
//                         },
//                         None => "".into(),
//                     }
//                 },

//                 TrackedParamType::Untracked(_) => unreachable!(),
//             }
//         })
//         .collect::<Vec<_>>()
//         .join("\n")
// }

// /// Creates a function stub string based off the passed in information.
// ///
// /// If the function is main, the stub includes the call to report all ATI information
// /// at the end of execution. If the function has a return value, the stub will make
// /// sure to include the RET variable in the exit site.
// fn create_fn_stub(
//     fn_name: &String,
//     inputs: &Vec<(String, TrackedParamType)>,
//     output: &Option<String>,
//     fn_sigs: &FunctionSignatures,
// ) -> String {
//     let param_decls = inputs
//         .iter()
//         .map(|(param_name, param_type)| {
//             let ptype = match param_type {
//                 TrackedParamType::Regular(t)
//                 | TrackedParamType::Reference(t, _)
//                 | TrackedParamType::FixedLengthArray(t, _)
//                 | TrackedParamType::Struct(t, _) => t,
//                 TrackedParamType::Untracked(t) => t,
//             };

//             format!("{param_name}: {ptype}")
//         })
//         .collect::<Vec<_>>()
//         .join(", ");
//     let enter_param_binds = create_site_binds("site_enter", inputs, fn_sigs);
//     let exit_param_binds = create_site_binds("site_exit", inputs, fn_sigs);
//     let params_passed = inputs
//         .iter()
//         .map(|(param_name, _)| param_name.clone())
//         .collect::<Vec<_>>()
//         .join(", ");

// }

// /// Uses previously discovered modified function information to define new "stub functions"
// /// which dynamically create *::ENTER and *::EXIT sites, and then invoke the "unstubbed"
// /// functions. Note that function stubs retain the original name of the function,
// /// so that any uses of that function automatically invoke our stub instead.
// pub fn create_stubs<'a>(krate: &mut ast::Crate, psess: &ParseSess, fn_sigs: &FunctionSignatures) {
//     for item in fn_sigs.create_stub_items(psess) {
//         krate.items.insert(0, item)
//     }
// }

/// Gives access to ATI types to all files being compiled
pub fn import_root_crate(krate: &mut ast::Crate, psess: &ParseSess) {
    let code = r#"
        use crate::*;
    "#;

    let items = common::parse_items(psess, code.into(), None);
    for item in items {
        krate.items.insert(0, item);
    }
}

// TODO FORMAT THIS WHOLE FILE BETTER
// could consider using a struct n defining everything on it to make fsigs easy to access
// could also just ONLY pass around fn_sigs <-- probably this
// AND THEN FINISH OUTPUTTING STRUCT STUFF STORED IN FUNCSIGS

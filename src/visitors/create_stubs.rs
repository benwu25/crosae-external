/* Creates function stubs for each tracked function discovered
 * by the visitor in params.rs. Each stub sets up enter and exit
 * sites before invoking the actual function.
*/
use rustc_ast as ast;

use rustc_session::parse::ParseSess;

use crate::common;
use crate::types::ati_info::FunctionSignatures;

fn create_site_binds<'a>(
    site_name: &str,
    inputs: impl Iterator<Item = &'a (String, bool, String)>,
) -> String {
    inputs
        .filter(|(_, is_tupled, _)| *is_tupled)
        .map(|(name, _, _)| {
            format!(
                r#"
            {site_name}.bind("{name}", {name});
        "#
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn create_fn_stub(
    fn_name: &String,
    inputs: &Vec<(String, bool, String)>,
    output: &Option<String>,
) -> String {
    let param_decls = inputs
        .iter()
        .map(|(param_name, _, param_type)| format!("{param_name}: {param_type}"))
        .collect::<Vec<_>>()
        .join(", ");
    let enter_param_binds = create_site_binds("site_enter", inputs.iter());
    let exit_param_binds = create_site_binds("site_exit", inputs.iter());
    // FIXME: not a great clone
    let params_passed = inputs
        .iter()
        .map(|(param_name, _, _)| param_name.clone())
        .collect::<Vec<_>>()
        .join(", ");

    if fn_name == "main" {
        // TODO: environment stuff for main
        // this is kind of a silly stub for now...
        format!(
            r#"
            pub fn main() {{
                let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site("main::ENTER");
                ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                main_unstubbed();

                let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("main::EXIT");
                ATI_ANALYSIS.lock().unwrap().update_site(site_exit);

                ATI_ANALYSIS.lock().unwrap().report();
            }}
        "#
        )
    } else if let Some(ret) = output {
        // with a return value
        format!(
            r#"
            pub fn {fn_name}({param_decls}) -> {ret} {{
                let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site("{fn_name}::ENTER");
                {enter_param_binds}
                ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                let res = {fn_name}_unstubbed({params_passed});

                let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{fn_name}::EXIT");
                {exit_param_binds}
                site_exit.bind("RET", res);
                ATI_ANALYSIS.lock().unwrap().update_site(site_exit);
                return res;
            }}
        "#
        )
    } else {
        // without a return value
        format!(
            r#"
            pub fn {fn_name}({param_decls}) {{
                let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site("{fn_name}::ENTER");
                {enter_param_binds}
                ATI_ANALYSIS.lock().unwrap().update_site(site_enter);

                {fn_name}_unstubbed({params_passed});

                let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site("{fn_name}::EXIT");
                {exit_param_binds}
                ATI_ANALYSIS.lock().unwrap().update_site(site_exit);
            }}
        "#
        )
    }
}

/// Uses previously discovered modified function information to define new "stub functions"
/// which dynamically create *::ENTER and *::EXIT sites, and then invoke the "unstubbed"
/// functions. Note that function stubs retain the original name of the function,
/// so that any uses of that function automatically invoke our stub instead.
pub fn create_stubs<'a>(krate: &mut ast::Crate, psess: &ParseSess, fn_sigs: &FunctionSignatures) {
    for (fn_name, (inputs, output)) in fn_sigs.iter() {
        let code = create_fn_stub(fn_name, inputs, output);

        let items = common::parse_items(psess, code, None);
        for item in items {
            krate.items.insert(0, item)
        }
    }
}

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

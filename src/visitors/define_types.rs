/* Defines a function which reads a file, parses it, and adds every single
 * type definition into the crate being instrumented. This is used to effectively
 * import a file at compile time, providing access to the necessary definitions to
 * perform ATI.
*/
use rustc_ast as ast;
use rustc_session::parse::ParseSess;

use crate::common;

// FIXME: should I make this an actual module import?? might lead to slightly cleaner code?

/// `file` must be a path to a .rs file containing required struct defs,
/// enum defs, and thier associated impl blocks, to be added to the target
/// program. Also handles use statements!
pub fn define_types_from_file(file: &std::path::Path, psess: &ParseSess, krate: &mut ast::Crate) {
    let code: String = std::fs::read_to_string(file).unwrap();

    let mut items = common::parse_items(psess, code, Some(file));
    items.sort_by(|a, b| match (&a.kind, &b.kind) {
        (ast::ItemKind::Use(_), ast::ItemKind::Use(_)) => std::cmp::Ordering::Equal,
        (_, ast::ItemKind::Use(_)) => std::cmp::Ordering::Less,
        (ast::ItemKind::Use(_), _) => std::cmp::Ordering::Greater,
        (_, _) => std::cmp::Ordering::Equal,
    });

    // actually add the stuff we've collected to the crate
    // placing imports above all other items
    for (i, item) in items.into_iter().enumerate() {
        krate.items.insert(i, item);
    }
}

// XX Tested with nightly-2025-03-28
// Tested with nightly-current.

#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_ast_pretty;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_error_codes;
extern crate rustc_errors;
//extern crate rustc_hash;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_parse;

use std::io;
use std::path::Path;
use std::sync::Arc;

use rustc_ast_pretty::pprust::item_to_string;
use rustc_driver::{Compilation, run_compiler};
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::ty::TyCtxt;
use rustc_ast::mut_visit;
use rustc_ast::mut_visit::*;

struct TestVisitor {}

impl MutVisitor for TestVisitor {
    fn visit_fn(&mut self, mut fk: FnKind<'_>, _attrs: &rustc_ast::AttrVec, _span: rustc_span::Span, _id: rustc_ast::NodeId) {
        println!("Hello fn!");

        mut_visit::walk_fn(self, fk);
    }
}


struct MyFileLoader {
    real_loader: rustc_span::source_map::RealFileLoader,
}

impl rustc_span::source_map::FileLoader for MyFileLoader {
    fn current_directory(&self) -> std::io::Result<std::path::PathBuf> {
        std::env::current_dir()
    }

    fn file_exists(&self, path: &Path) -> bool {
        //path == Path::new("main.rs")
        self.real_loader.file_exists(path)
    }

    fn read_file(&self, path: &Path) -> io::Result<String> {
        /* if path == Path::new("main.rs") {
            Ok(r#"
fn main() {
    let message = "Hello, World!";
    println!("{message}");
}
"#
            .to_string())
        } else {
            Err(io::Error::other("oops"))
        } */

        // Instrument and pretty-print back.

        // Steps: read file into a String, create a parser from the string, parse the stuff into a
        // krate, and visit_crate using your DaikonDtraceVisitor.
        //let contents = std::fs::read_to_string(path).unwrap();

        let contents = self.real_loader.read_file(path).unwrap();
        let psess = rustc_session::parse::ParseSess::new();
        let mut tmp_parser = rustc_parse::new_parser_from_source_str(
            &psess,
            rustc_span::FileName::Custom(String::from("parserX")),
            contents.clone(),
            rustc_parse::lexer::StripTokens::Nothing,
        ).unwrap();
        let mut file_ast = tmp_parser.parse_crate_mod().unwrap();
        let mut test_visitor = TestVisitor {};
        mut_visit::walk_crate(&mut test_visitor, &mut file_ast);

        Ok(contents)
    }

    fn read_binary_file(&self, path: &Path) -> io::Result<Arc<[u8]>> {
        //Err(io::Error::other("oops"))
        self.real_loader.read_binary_file(path)
    }
}

struct MyCallbacks;

impl rustc_driver::Callbacks for MyCallbacks {
    fn config(&mut self, config: &mut Config) {
        config.file_loader = Some(Box::new(MyFileLoader { real_loader: rustc_span::source_map::RealFileLoader }));
    }

    fn after_crate_root_parsing(
        &mut self,
        _compiler: &Compiler,
        krate: &mut rustc_ast::Crate,
    ) -> Compilation {
        //for item in &krate.items {
            //println!("{}", item_to_string(&item));
        //}

        Compilation::Continue
    }

    fn after_analysis(&mut self, _compiler: &Compiler, _tcx: TyCtxt<'_>) -> Compilation {
        // Iterate over the top-level items in the crate, looking for the main function.
        /*for id in tcx.hir_free_items() {
            let item = &tcx.hir_item(id);
            // Use pattern-matching to find a specific node inside the main function.
            if let rustc_hir::ItemKind::Fn { body, .. } = item.kind {
                let expr = &tcx.hir_body(body).value;
                if let rustc_hir::ExprKind::Block(block, _) = expr.kind {
                    if let rustc_hir::StmtKind::Let(let_stmt) = block.stmts[0].kind {
                        if let Some(expr) = let_stmt.init {
                            let hir_id = expr.hir_id; // hir_id identifies the string "Hello, world!"
                            let def_id = item.hir_id().owner.def_id; // def_id identifies the main function
                            let ty = tcx.typeck(def_id).node_type(hir_id);
                            println!("{expr:#?}: {ty:?}");
                        }
                    }
                }
            }
        } */

        //Compilation::Stop
        Compilation::Continue
    }
}

fn main() {
    // change this: forward our command line args to run_compiler.
    let args: Vec<String> = std::env::args().collect();
    run_compiler(&args, &mut MyCallbacks);
/*    run_compiler(
        &[
            // The first argument, which in practice contains the name of the binary being executed
            // (i.e. "rustc") is ignored by rustc.
            "ignored".to_string(),
            "main.rs".to_string(),
        ],
        &mut MyCallbacks,
    ); */
}

// Tested with nightly-current (04/24/2026)

#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_ast_pretty;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_error_codes;
extern crate rustc_errors;
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
        self.real_loader.file_exists(path)
    }

    fn read_file(&self, path: &Path) -> io::Result<String> {

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
        // return instrumented pretty-printed ast
        let mut instrumented_pretty_printed_file = String::new();
        for file_item in &file_ast.items {
            instrumented_pretty_printed_file.push_str(&item_to_string(&file_item));
            instrumented_pretty_printed_file.push_str("\n\n");
        }

        Ok(instrumented_pretty_printed_file)
    }

    fn read_binary_file(&self, path: &Path) -> io::Result<Arc<[u8]>> {
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
        Compilation::Continue
    }

    fn after_analysis(&mut self, _compiler: &Compiler, _tcx: TyCtxt<'_>) -> Compilation {
        Compilation::Continue
    }
}

fn main() {
    // change this: forward our command line args to run_compiler.
    let args: Vec<String> = std::env::args().collect();
    run_compiler(&args, &mut MyCallbacks);
}

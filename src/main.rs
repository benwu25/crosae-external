/* Entry point file for DATIR.
 * This file defines the callbacks that are then passed to the rustc_driver 
 * invocation in main. View the `Callbacks` struct below, which currently only
 * takes advantage of a single callback function, for more information.
*/
#![feature(rustc_private)]
#![feature(box_patterns)]

// It's okay if rust-analyzer is struggling to resolve these crates.
// If you followed the direction in the README to add the necessary rustup
// components, everything should work fine!

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_hir;
extern crate rustc_parse;
extern crate rustc_session;
extern crate rustc_span;

use rustc_ast as ast;
use rustc_ast::mut_visit::MutVisitor;
use rustc_driver::Compilation;
use rustc_interface::interface;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

use std::{collections::{HashMap, HashSet}, env};
use rustc_span::def_id::DefId;

mod instrumentation;
use crate::instrumentation::{
    FindCallsVisitor, TupleLiteralsVisitor, UpdateFnDeclsVisitor, create_stubs, define_types_from_file
};

// included just for code analysis to run on ati.rs
mod ati;

struct InstrumentationCallbacks {
    pub call_spans: HashMap<Span, String>,
}
impl rustc_driver::Callbacks for InstrumentationCallbacks {
    /// Called before creating the compiler instance
    fn config(&mut self, _config: &mut interface::Config) {}

    /// Called after parsing the crate root. Submodules are not yet parsed when
    /// this callback is called. Return value instructs the compiler whether to
    /// continue the compilation afterwards (defaults to `Compilation::Continue`)
    fn after_crate_root_parsing(
        &mut self,
        compiler: &interface::Compiler,
        krate: &mut ast::Crate,
    ) -> Compilation {
        // discovers all functions that will be instrumented, and updates
        // the function signatures to tag all passed values as necessary.
        // also updates type definitions in structs. 
        let mut modify_params_visitor = UpdateFnDeclsVisitor::new();
        modify_params_visitor.visit_crate(krate);
        let modified_funcs = modify_params_visitor.get_modified_funcs();

        // tuple all literals to create tags, untupling as necessary
        // when they are passed into untracked functions
        let mut visitor = TupleLiteralsVisitor::new(modified_funcs, &self.call_spans);
        visitor.visit_crate(krate);

        // create all required function stubs, which perform site management
        create_stubs(krate, &compiler.sess.psess, modified_funcs);

        // define all used ATI types from ati.rs
        // do this last so that instrumentation is not applied to these types
        let cwd = std::env::current_dir().unwrap();
        define_types_from_file(
            &cwd.join("src/ati/ati.rs"),
            &compiler.sess.psess,
            krate,
        );

        Compilation::Continue
    }

    // leaving the other callbacks just in case they are useful
    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        _tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        Compilation::Continue
    }

    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        _tcx: TyCtxt<'tcx>,
    ) -> Compilation {

        Compilation::Continue
    }
}

struct TypingCallbacks {
    /// for ident matching?
    /// let def_path = tcx.def_path_str(def_id);
    fn_defs: HashSet<DefId>,
    call_spans: Option<HashMap<Span, String>>,
}
impl<'a> rustc_driver::Callbacks for TypingCallbacks {
    /// Called before creating the compiler instance
    fn config(&mut self, config: &mut interface::Config) {
        config.opts.unstable_opts.no_codegen = true;
    }

    /// Called after parsing the crate root. Submodules are not yet parsed when
    /// this callback is called. Return value instructs the compiler whether to
    /// continue the compilation afterwards (defaults to `Compilation::Continue`)
    fn after_crate_root_parsing(
        &mut self,
        _compiler: &interface::Compiler,
        _krate: &mut ast::Crate,
    ) -> Compilation {
        Compilation::Continue
    }

    // leaving the other callbacks just in case they are useful
    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        // discover all function {defs} and {calls}, split into tracked/untracked ({defs} / {calls}\{defs})

        for local_def_id in tcx.hir_body_owners() {
            let def_id = local_def_id.to_def_id();
            self.fn_defs.insert(def_id);
        }

        let mut find_calls_visitor = FindCallsVisitor{ 
            tcx,
            // FIXME: this really shouldn't be a clone
            defs: self.fn_defs.clone(),
            call_spans: HashMap::new(),
        };
        tcx.hir_walk_toplevel_module(&mut find_calls_visitor);
        // println!("defs:  {:?}", find_calls_visitor.defs);
        // println!("calls: {:?}", find_calls_visitor.call_spans);

        self.call_spans.replace(find_calls_visitor.call_spans);

        Compilation::Continue
    }

    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        _tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        // find spans which map to types at "relevant locations"
        // relevant locations are places where an untracked function was called
        // find spans 
        Compilation::Continue
    }
}

/// Entry-point, forwards all arguments command line arguments to rustc_driver
pub fn main() {
    let args: Vec<_> = env::args().collect();

    let mut typing = TypingCallbacks {
        fn_defs: HashSet::new(),
        call_spans: None,
    };
    rustc_driver::run_compiler(&args, &mut typing);

    let mut instr = InstrumentationCallbacks {
        call_spans: typing.call_spans.take().unwrap(),
    };
    rustc_driver::run_compiler(&args, &mut instr);
}

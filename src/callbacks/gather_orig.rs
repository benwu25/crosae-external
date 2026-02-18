/* Before we can perform the required AST mutation, we need to gather
 * some type information about the original source code. This is done by
 * invoking the compiler once, passing in the callback struct defined in
 * this file, before invoking the compiler again with the mutation callbacks.
*/
use rustc_ast as ast;
use rustc_driver::Compilation;
use rustc_interface::interface;
use rustc_middle::ty::TyCtxt;

use crate::{types::ati_info::FunctionBoundaries, visitors::FindUntrackedCallsVisitor};

/// Contains the callbacks used for the first information-gathering compilation.
pub struct GatherAtiInfo {
    /// contains the information discovered after executing the compilation.
    fbs: Option<FunctionBoundaries>,
}

impl GatherAtiInfo {
    pub fn new() -> Self {
        Self { fbs: None }
    }

    /// Pulls out all gathered info that this compiler invocation learned.
    /// Panics if this function is called before the pass is performed.
    pub fn pull_function_boundaries(&mut self) -> FunctionBoundaries {
        self.fbs.take().unwrap()
    }
}

impl<'a> rustc_driver::Callbacks for GatherAtiInfo {
    /// disables everything after MIR construction
    fn config(&mut self, config: &mut interface::Config) {
        config.opts.unstable_opts.no_codegen = true;
    }

    fn after_crate_root_parsing(
        &mut self,
        _compiler: &interface::Compiler,
        _krate: &mut ast::Crate,
    ) -> Compilation {
        Compilation::Continue
    }

    /// Finds all functions that require tracking, alongside
    /// the code locations of any untracked function invocations.
    /// Populates self.fbs with that information.
    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        let mut fbs = FunctionBoundaries::new();

        // find all user-defined functions
        // TODO: i'm not sure what this is going to do with closures
        for local_def_id in tcx.hir_body_owners() {
            let node = tcx.hir_node_by_def_id(local_def_id);
            if let rustc_hir::Node::Item(rustc_hir::Item {
                kind: rustc_hir::ItemKind::Fn { ident, .. },
                ..
            }) = node
            {
                fbs.observe_tracked_fn(&ident, local_def_id.to_def_id());
            } else if let rustc_hir::Node::AnonConst(anon_const) = node {
                // chill to just ignore?
            } else {
                panic!(
                    "Found body owner that isn't a function while discovering ATI info: {node:#?}"
                )
            }
        }

        // find all places where a non-user-defined function was called
        let mut find_calls_visitor = FindUntrackedCallsVisitor { tcx, fbs: &mut fbs };
        tcx.hir_walk_toplevel_module(&mut find_calls_visitor);

        self.fbs = Some(fbs);
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

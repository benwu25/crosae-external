/* This file defines the callbacks used by the second compiler invocation,
 * which is responsible for actually modifying the AST to include required
 * instrumentation calls and site management.
*/
use rustc_ast as ast;
use rustc_driver::Compilation;
use rustc_interface::interface;
use rustc_middle::ty::TyCtxt;

use crate::{
    file_loaders::transforming_loader::TransformingFileLoader, types::ati_info::FunctionBoundaries,
    visitors::define_types_from_file,
};

/// Callbacks struct to be passed into the compiler invocation.
/// Notably, this pass comes after some function type information was discovered
/// by running a different pass and querying the HIR. This information is
/// passed via the `fbs` field.
pub struct InstrumentAti {
    fbs: Option<FunctionBoundaries>,
}
impl InstrumentAti {
    pub fn new(fbs: FunctionBoundaries) -> Self {
        Self { fbs: Some(fbs) }
    }
}

impl rustc_driver::Callbacks for InstrumentAti {
    /// Configures the compiler invocation to use the TransformingFileLoader,
    /// which is responsible for actually performing the necessary AST mutation.
    fn config(&mut self, config: &mut interface::Config) {
        // use our custom loader to also instrument non-root files
        // this loader will be the one responsible for adding all stubs,
        // tupling all literals, etc.
        config.file_loader = Some(Box::new(TransformingFileLoader::new(
            self.fbs.take().unwrap(),
        )));
    }

    /// Define necessary types in the root file. All other files will
    /// import these types from the root.
    fn after_crate_root_parsing(
        &mut self,
        compiler: &interface::Compiler,
        krate: &mut ast::Crate,
    ) -> Compilation {
        let cwd = std::env::current_dir().unwrap();
        define_types_from_file(&cwd.join("src/ati/ati.rs"), &compiler.sess.psess, krate);

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

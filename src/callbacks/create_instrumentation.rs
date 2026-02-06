use rustc_ast as ast;
use rustc_driver::Compilation;
use rustc_interface::interface;
use rustc_middle::ty::TyCtxt;

use crate::{
    file_loaders::transforming_loader::TransformingFileLoader, types::ati_info::FunctionBoundaries,
    visitors::define_types_from_file,
};

pub struct InstrumentAti {
    fbs: Option<FunctionBoundaries>,
}
impl InstrumentAti {
    pub fn new(fbs: FunctionBoundaries) -> Self {
        Self { fbs: Some(fbs) }
    }
}

impl rustc_driver::Callbacks for InstrumentAti {
    /// Called before creating the compiler instance
    fn config(&mut self, config: &mut interface::Config) {
        // use our custom loader to also instrument non-root files
        // this loader will be the one responsible for adding all stubs,
        // tupling all literals, etc.
        config.file_loader = Some(Box::new(TransformingFileLoader::new(
            self.fbs.take().unwrap(),
        )));
    }

    /// Called after parsing the crate root. Submodules are not yet parsed when
    /// this callback is called. Return value instructs the compiler whether to
    /// continue the compilation afterwards (defaults to `Compilation::Continue`)
    fn after_crate_root_parsing(
        &mut self,
        compiler: &interface::Compiler,
        krate: &mut ast::Crate,
    ) -> Compilation {
        // define all used ATI types from ati.rs
        // these type are all defined in the root file, then imported in all others
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

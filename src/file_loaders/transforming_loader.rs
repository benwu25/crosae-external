/* This file defines a TransformingFileLoader. This struct can be used in place
 * of the regular file loader used by rustc to read in any file it is about to compile
 * by setting `config.file_loader` (easiest way to do that is by using the `config()`
 * callback). This custom loader allows for using the regular AST visitor pattern but to
 * mutate *any* file, and not just the root.
*/
use rustc_ast as ast;
use rustc_ast::mut_visit::MutVisitor;
use rustc_ast_pretty::pprust;
use rustc_session::parse::ParseSess;
use rustc_span::source_map::{FileLoader, RealFileLoader};
use std::io;
use std::path::Path;

use crate::common;
use crate::types::ati_info::FunctionBoundaries;
use crate::visitors::{TupleLiteralsVisitor, UpdateFnDeclsVisitor, import_root_crate};

/// This FileLoader constructs an early intermediate AST of any file that is loaded
/// through it. This intermediate AST can be modified using the regular Visitor
/// pattern, before being written back into a string format and actually handed
/// off to the rest of rustc. Rustc will then reconstruct this same AST, which is
/// quite unfortunate. But, this overall allows for making AST modifications
/// to files which are not just the crate root using the same infrastructure as the
/// rest of the compiler.
// NIT: would be nice to make this accept a list of visitors to execute
pub struct TransformingFileLoader {
    /// The regular FileLoader that rustc uses
    inner: RealFileLoader,
    /// Information regarding tracked/untracked function calls
    /// that was discovered by some prior queries of the HIR.
    fbs: FunctionBoundaries,
}

/// Represents the string contents of a file, alongside the type of file
type FileContents = (String, FileType);
#[derive(Debug)]
enum FileType {
    /// Represents the tracked crate root file.
    Root,
    /// Represents a tracked dep file.
    Dep,
    /// Represents an untracked file.
    Untracked,
}

impl TransformingFileLoader {
    /// Constructor
    pub fn new(fbs: FunctionBoundaries) -> Self {
        Self {
            inner: RealFileLoader,
            fbs: fbs,
        }
    }

    /// Creates a new parser
    fn create_parse_sess() -> ParseSess {
        ParseSess::new(Vec::from([rustc_driver::DEFAULT_LOCALE_RESOURCE]))
    }

    /// Transforms the file at `path` which contains `contents`, by first parsing
    /// the contents into an AST, performing visitor passes over the AST,
    /// then converting the transformed AST back into a file contents string.
    /// This effectively inserts a second AST construction before the actual rustc AST
    /// is made. Thats unfortunate, but necessary as there is no callback that gets
    /// invoked for each non-crate-root file that is parsed in.
    fn transform_source(&self, contents: FileContents, path: &Path) -> String {
        let psess = Self::create_parse_sess();
        let (contents, file_type) = contents;

        let mut krate = common::parse_crate(&psess, contents, Some(path));

        // tuple all literals to create tags, untupling them as necessary
        // when they are passed into untracked functions, and further re-tupling returns
        // from those untracked functions if they return trackable types.
        let mut tl_vis = TupleLiteralsVisitor::new(&self.fbs);
        tl_vis.visit_crate(&mut krate);

        // discovers all functions that will be instrumented, and updates
        // the function signatures to tag all passed-in params, if necessary.
        // also updates type definitions in structs to have fields be tagged.
        let mut fn_decls_vis = UpdateFnDeclsVisitor::new(&self.fbs);
        fn_decls_vis.visit_crate(&mut krate);

        // create all required function stubs, which perform site management
        let fn_sigs = fn_decls_vis.get_new_fn_signatures();
        fn_sigs.create_stub_items(&mut krate, &psess);
        // create_stubs(&mut krate, &psess, &fn_sigs);

        // make the ATI types available to dependancies
        if matches!(file_type, FileType::Dep) {
            import_root_crate(&mut krate, &psess);
        }

        self.ast_to_source(&krate)
    }

    /// Converts an Crate AST to a standard string representation, equivalent
    /// to that of a regular source file. After this call, the regular rustc
    /// parser will be ready to run again consuming the output string.
    fn ast_to_source(&self, krate: &ast::Crate) -> String {
        let mut output = String::new();

        // probably unnecessary right now, but these are the only "other thing"
        // in the krate that is found in the source file.
        for attr in &krate.attrs {
            let attr_str = pprust::attribute_to_string(attr);
            output.push_str(&attr_str);
            output.push('\n');
        }

        for item in &krate.items {
            let item_str = pprust::item_to_string(&item);
            output.push_str(&item_str);
            output.push_str("\n\n"); // two \n just to match normal file loader
        }

        println!("INSTRUMENTED:\n{output}");

        output
    }

    /// Reads in a file at `path` directly, while also determining what kind of
    /// file it is (Root, Dep, or Untracked)
    fn read_file(&self, path: &Path) -> io::Result<FileContents> {
        let contents = self.inner.read_file(path)?;
        let path_str = path.to_str().unwrap_or("");

        // non .rs files, or std library files, external crates, etc.
        let file_type = if path.extension().and_then(|s| s.to_str()) != Some("rs")
            || path_str.contains("/.rustup/")
            || path_str.contains("/.cargo/")
            || path_str.contains("/rustc/")
        {
            FileType::Untracked
        } else if path_str.ends_with("main.rs") || path_str.ends_with("lib.rs") {
            // TODO: how do we know a file is the root based off the path?
            // as in, calling rustc <file> makes <file> the root, regardless of name.
            // do we need to use an ENV var or something? does rustc happen to set one?
            FileType::Root
        } else {
            FileType::Dep
        };

        Ok((contents, file_type))
    }
}

/// Implements the necessary trait to use the custom loader as a
/// file loader in the compiler.
impl FileLoader for TransformingFileLoader {
    /// Returns true if the file at `path` exists
    fn file_exists(&self, path: &Path) -> bool {
        self.inner.file_exists(path)
    }

    /// Reads the file point to by `path` into a String. This function
    /// will actually do the transformations defined in the TransformingFileLoader.
    fn read_file(&self, path: &Path) -> io::Result<String> {
        let file_contents = self.read_file(path)?;

        // If we ever read in a file that we are not instrumenting,
        // then just pass the contents up, skipping the transformation step.
        if matches!(file_contents.1, FileType::Untracked) {
            Ok(file_contents.0)
        } else {
            Ok(self.transform_source(file_contents, path))
        }
    }

    // Would we ever do this? I guess if we do like extern linking? idk when this is invoked.
    fn read_binary_file(&self, path: &Path) -> io::Result<std::sync::Arc<[u8]>> {
        unimplemented!()
    }

    /// Gets the current directory.
    fn current_directory(&self) -> io::Result<std::path::PathBuf> {
        std::env::current_dir()
    }
}

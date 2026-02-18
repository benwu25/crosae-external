/* Entry point file for DATIR.
 * This file defines the callbacks that are then passed to the rustc_driver
 * invocation in main. View the `Callbacks` struct below, which currently only
 * takes advantage of a single callback function, for more information.
*/
#![feature(rustc_private)]
#![feature(box_patterns)]

extern crate rustc_ast;
extern crate rustc_ast_pretty;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_parse;
extern crate rustc_session;
extern crate rustc_span;

use std::env;

mod ati; // included just for code analysis to run on ati.rs
mod callbacks;
mod common;
mod file_loaders;
mod types;
mod visitors;

/// Entry-point, forwards all command-line arguments to rustc_driver
pub fn main() {
    let args: Vec<_> = env::args().collect();

    let mut gather_info = callbacks::gather_orig::GatherAtiInfo::new();
    rustc_driver::run_compiler(&args, &mut gather_info); // panics on compilation failure
    let fbs = gather_info.pull_function_boundaries();

    let mut instr = callbacks::create_instrumentation::InstrumentAti::new(fbs);
    rustc_driver::run_compiler(&args, &mut instr);
}

use std::collections::VecDeque;
use std::fmt::Write;
use std::io::Write as FileWrite;
use std::mem;
use std::sync::{LazyLock, Mutex};

use rustc_data_structures::thin_vec::{ThinVec, thin_vec};
use rustc_data_structures::fx::FxHashMap;
use rustc_ast::*;
use rustc_ast::mut_visit;
use rustc_ast::mut_visit::*;
use rustc_parse::{unwrap_or_emit_fatal, new_parser_from_source_str};
use rustc_parse::lexer::StripTokens;
use rustc_parse::parser::{AllowConstBlockItems, ForceCollect};
use rustc_ast_pretty::pprust;
use rustc_session::parse::ParseSess;
use rustc_errors::PResult;
use crate::daikon_strs::*;
use crate::dtrace_routine_builders::*;

// change this (set properly)
pub static OUTPUT_PREFIX: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::from("main")));
static PARSER_COUNTER: LazyLock<Mutex<u32>> = LazyLock::new(|| Mutex::new(0));

// Represents a scope that we are instrumenting/visiting,
// see DaikonDtraceVisitor::scope_stack member. If we are
// visiting a struct in the toplevel scope, the top of the
// scope_stack will be a ScopeType::TopLevelScope. The new
// synthesized impl block will be placed in the ScopeType.
// For structs encountered in nested scopes, such as a
// function body, those impls will be placed in a different
// ScopeType representing the current function body, so they
// are placed in the correct function scope.
pub enum ScopeType {
    ToplevelScope(ThinVec<Box<Item>>),
    FnBody(ThinVec<Stmt>),
    // [continues for whatever can have Items, and thus struct definitions]
}

/*
   Primary visitor pass for dtrace instrumentation.
*/
pub struct DaikonDtraceVisitor<'a> {
    // For parsing string fragments.
    pub psess: &'a ParseSess,

    // For adding impl blocks which bundle dtrace_* routines.
    pub scope_stack: &'a mut VecDeque<ScopeType>,
}

// Represents a Rust type.
// If it is a reference, note this with is_ref in get_rust_type.
// For Vec/array, is_ref indicates whether the contents of the
// container are references or not rather than the container
// itself. It does not matter if the container is_ref or not,
// since we always make a copy Vec with references to contents.
// E.g.,
// i32 -> Prim("i32")
// Vec<char> -> PrimVec("char")
// &'a Vec<X> -> UserDefVec("X")
// &[String] -> PrimArray("String")
// &'a &'b Widget -> UserDef("Widget")
// All enums, structs, and unions are categorized as UserDef,
// so we cannot distinguish between them after this point,
// and this forces us to implement noop dtrace routines
// for them.
// This can be fixed by doing a first pass to filter only
// structs which belong to the crate being compiled. Then
// we can appeal to a /tmp file at compile-time and skip
// enums, unions, and all UserDef types from outside the
// crate.
#[derive(PartialEq)]
enum RustType {
    Prim(String),
    UserDef(String),
    PrimVec(String),
    UserDefVec(String),
    PrimArray(String),
    UserDefArray(String),
    Skip,  // Types we want to skip.
    NoRet, // For void-returning functions.
    Error, // Used to indicate a type is not primitive.
}

// Convert a Pat representing a parameter name into a String representation.
// * `pat` - Pat struct representing an identifier for a parameter.
fn get_param_ident(pat: &Box<Pat>) -> String {
    match &pat.kind {
        PatKind::Ident(_mode, ident, None) => String::from(ident.as_str()),
        _ => panic!("Parameter does not have simple identifier"),
    }
}

// Given a type, check if the type is a primitive and return a RustType
// representing it or RustType::Error otherwise.
// i32 -> RustType::Prim("i32")
// Vec<X> -> RustType::Error
// * `ty_str` - String representing a parameter type.
fn as_primitive(ty_str: &str) -> RustType {
    if ty_str == I8 {
        return RustType::Prim(String::from(I8));
    } else if ty_str == I16 {
        return RustType::Prim(String::from(I16));
    } else if ty_str == I32 {
        return RustType::Prim(String::from(I32));
    } else if ty_str == I64 {
        return RustType::Prim(String::from(I64));
    } else if ty_str == I128 {
        return RustType::Prim(String::from(I128));
    } else if ty_str == ISIZE {
        return RustType::Prim(String::from(ISIZE));
    } else if ty_str == U8 {
        return RustType::Prim(String::from(U8));
    } else if ty_str == U16 {
        return RustType::Prim(String::from(U16));
    } else if ty_str == U32 {
        return RustType::Prim(String::from(U32));
    } else if ty_str == U64 {
        return RustType::Prim(String::from(U64));
    } else if ty_str == U128 {
        return RustType::Prim(String::from(U128));
    } else if ty_str == USIZE {
        return RustType::Prim(String::from(USIZE));
    } else if ty_str == F32 {
        return RustType::Prim(String::from(F32));
    } else if ty_str == F64 {
        return RustType::Prim(String::from(F64));
    } else if ty_str == CHAR {
        return RustType::Prim(String::from(CHAR));
    } else if ty_str == BOOL {
        return RustType::Prim(String::from(BOOL));
    } else if ty_str == UNIT {
        return RustType::Prim(String::from(UNIT));
    } else if ty_str == STR {
        return RustType::Prim(String::from(STR));
    } else if ty_str == STRING {
        return RustType::Prim(String::from(STRING));
    }
    RustType::Error
}

// Given the type of the object contained in a Vec,
// represented as a Path struct, return a RustType representing
// the Vec.
// is_ref is set to true if the Vec contains references.
// Vec<X> -> UserDefVec("X")
// &'a Vec<X> -> UserDefVec("X")
// Vec<&X> -> UserDefVec("X"), is_ref == true
// &Vec<X> -> UserDefVec("X")
// &Vec<&X> -> UserDefVec("X"), is_ref == true
// * `generic_args` - Generic args to a Vec parameter.
fn vec_generics_to_rust_type(generic_args: &Path, is_ref: &mut bool) -> RustType {
    // Reset in case we have an &Vec<X>, since we want to know if
    // the Vec arguments are references are not, i.e., Vec<X> vs.
    // Vec<&X>.
    *is_ref = false;
    match &generic_args.segments[generic_args.segments.len() - 1].args {
        None => RustType::Error,
        Some(args) => match &**args {
            GenericArgs::AngleBracketed(brack_args) => match &brack_args.args[0] {
                AngleBracketedArg::Arg(arg) => match &arg {
                    GenericArg::Type(arg_type) => match &get_rust_type(&arg_type.kind, is_ref) {
                        RustType::Prim(p_type) => RustType::PrimVec(String::from(p_type)),
                        RustType::UserDef(basic_type) => {
                            RustType::UserDefVec(String::from(basic_type))
                        }
                        _ => RustType::Error,
                    },
                    _ => RustType::Error,
                },
                _ => RustType::Error,
            },
            _ => RustType::Error,
        },
    }
}

// Set global variable OUTPUT_PREFIX using input file path.
// If there is no output file specified with -o and we have not
// been invoked by cargo, take the OUTPUT_PREFIX from the input file
// name.
// foo.rs -> foo
// * `input_name` - Name of the program, i.e., a file prefix or crate name.
pub fn set_output_prefix(input_name: String) {
    let dot_idx = match input_name.rfind(".") {
        // .rs
        None => panic!("no '.' at the end of input file name {}", input_name),
        Some(end) => end,
    };
    let slash_idx = match input_name.rfind("/") {
        // .../<crate>.rs
        None => 0,
        Some(slash) => slash + 1,
    };
    let res = &input_name[slash_idx..dot_idx];
    *OUTPUT_PREFIX.lock().unwrap() = String::from(res);
}

// Create a RustType for the given Rust type.
// * `kind` - Represents the actual type of a parameter in the Rust language.
// * `is_ref` - Used to determine reference qualifiers on the type.
fn get_rust_type(kind: &TyKind, is_ref: &mut bool) -> RustType {
    match &kind {
        TyKind::Array(arr_type, _anon_const) => match &get_rust_type(&arr_type.kind, is_ref) {
            RustType::Prim(p_type) => RustType::PrimArray(String::from(p_type)),
            RustType::UserDef(basic_type) => RustType::UserDefArray(String::from(basic_type)),
            _ => panic!("higher-dim arrays not supported"),
        },
        TyKind::Slice(arr_type) => match &get_rust_type(&arr_type.kind, is_ref) {
            RustType::Prim(p_type) => RustType::PrimArray(String::from(p_type)),
            RustType::UserDef(basic_type) => RustType::UserDefArray(String::from(basic_type)),
            _ => panic!("higher-dim arrays not supported"),
        },
        // FIXME: implement logging and handling for Rust pointers.
        TyKind::Ptr(_mut_ty) => RustType::Error,
        TyKind::Ref(_, mut_ty) => {
            *is_ref = true;
            // recurse to get to the underlying type
            get_rust_type(&mut_ty.ty.kind, is_ref)
        }
        TyKind::Path(_, path) => {
            if path.segments.is_empty() {
                panic!("Path has no type");
            }
            let ty_string = path.segments[path.segments.len() - 1].ident.as_str();
            let try_prim = as_primitive(ty_string);
            if try_prim != RustType::Error {
                return try_prim;
            }
            if ty_string == VEC {
                return vec_generics_to_rust_type(&path, is_ref);
            }
            // Return full type: RustType<args>, need generics in some cases.
            RustType::UserDef(ty_string.to_string())
        }
        TyKind::ImplTrait(_, _) => {
            // A bunch of types we want to ignore for Daikon.
            RustType::Skip
        }
        _ => RustType::Error,
    }
}

// FIXME: replace this idea with better data structures for the logging code.
// Unused. This was intended to allow easy invalidation
// of parameters. E.g., if parameter x was invalidated with
// drop(x), we need to know which idx it belongs to in our
// Vec of dtrace information to avoid logging it at future
// exit ppts.
// Parameter invalidation is still unimplemented.
fn map_params(decl: &Box<FnDecl>) -> FxHashMap<String, i32> {
    let mut res = FxHashMap::default();
    for i in 0..decl.inputs.len() {
        res.insert(get_param_ident(&decl.inputs[i].pat), i as i32);
    }
    res
}

// Returns true if the last statement in each function is an
// explicit void return.
// Note: returns false in a case like the following:
/*
if cond { return; } else { return; }
*/
// In this case, an extra void return is unreachable.
// FIXME: handle checking for exhaustive control flow with
// explicit void returns.
// * `block` - A block representing a function body to check.
fn last_stmt_is_void_return(block: &Box<Block>) -> bool {
    if block.stmts.is_empty() {
        panic!("no stmts to check");
    }
    match &block.stmts[block.stmts.len() - 1].kind {
        StmtKind::Semi(semi) => match &semi.kind {
            ExprKind::Ret(None) => true,
            _ => false,
        },
        _ => false,
    }
}

impl<'a> DaikonDtraceVisitor<'a> {
    // Given a block of stmts in a String and a block, parse the string
    // and append parsed stmts to the end of the block.
    // * `to_insert` - A block of code to parse, either wrapped in a
    //                 function or a plain semi-colon separated sequence
    //                 of stmts.
    // * `block` - The block to append stmts to.
    fn append_to_block(&self, to_insert: &str, block: &mut Box<Block>) {
        match &self.parse_items_from_source_str(to_insert) {
            Err(_why) => panic!("Parsing internal String failed"),
            Ok(items) => match &items[0].kind {
                ItemKind::Fn(wrapper) => match &wrapper.body {
                    None => panic!("No body to insert"),
                    Some(body) => {
                        for stmt in body.stmts.clone() {
                            block.stmts.push(stmt.clone());
                        }
                    }
                },
                _ => panic!("Expected Fn in append_to_block"),
            },
        }
    }

    // Given a block of stmts in a String, a block, and an idx into the block,
    // parse the string and insert parsed stmts at the specified index.
    // * `loc` - A stmt index to insert new stmts.
    // * `to_insert` - A block of code to parse, either wrapped in a
    //                 function or a semi-colon separated sequence
    //                 of stmts.
    // * `block` - The block  to insert stmts into.
    fn insert_into_block(&self, loc: usize, to_insert: &str, block: &mut Box<Block>) -> usize {
        let mut i = loc;
        let items = self.parse_items_from_source_str(to_insert);
        match &items {
            Err(_why) => panic!("Internal String parsing failed"),
            Ok(items) => match &items[0].kind {
                ItemKind::Fn(wrapper) => match &wrapper.body {
                    None => panic!("No body to insert"),
                    Some(body) => {
                        for stmt in body.stmts.clone() {
                            block.stmts.insert(i, stmt.clone());
                            i += 1;
                        }
                    }
                },
                _ => panic!("Internal Daikon str is malformed"),
            },
        }
        i
    }

    // Pop stmts (impls) at the back into the front of stmts
    fn pop_back_into_stmts_front(&mut self, stmts: &mut ThinVec<Stmt>) {
        match self.scope_stack.pop_back() {
            Some(scope_type) => match scope_type {
                ScopeType::FnBody(stmt_vec) => {
                    for stmt in stmt_vec {
                        stmts.insert(0, stmt);
                    }
                }
                // [continues]
                _ => panic!("Expected a ScopeType with a stmt_vec"),
            },
            None => panic!("Expected scope_stack to be non-empty"),
        };
    }

    // Pop items (impls) at back into items
    pub fn pop_back_into_items(&mut self, items: &mut ThinVec<Box<Item>>) {
        match self.scope_stack.pop_back() {
            Some(scope_type) => match scope_type {
                // ScopeType::InlineMod(item_vec) |
                ScopeType::ToplevelScope(item_vec) => {
                    for item in item_vec {
                        items.push(item);
                    }
                }
                // [continues]
                ScopeType::FnBody(_) => panic!("Why is this an FnBody?"), //_ => panic!("Expected a ScopeType with an item_vec")
            },
            None => panic!("Expected scope_stack to be non-empty"),
        };
    }

    // Return true if we should call push_impl_item instead of
    // push_impl_stmt.
    fn scope_type_requires_items_p(&self) -> bool {
        match &self.scope_stack.back() {
            Some(back) => match &back {
                // ScopeType::InlineMod(_) |
                ScopeType::ToplevelScope(_) => {
                    // [continues]
                    true
                }
                _ => false,
            },
            None => false,
        }
    }

    // Push a new impl as a Stmt to the current scope
    fn push_impl_stmt(&mut self, impl_stmt: Stmt) {
        match &mut self.scope_stack.back_mut() {
            Some(back) => match back {
                ScopeType::FnBody(stmt_vec) => {
                    stmt_vec.push(impl_stmt);
                }
                // [continues]
                _ => panic!("Expected a ScopeType with stmts"),
            },
            None => panic!("Expected scope_stack non-empty"),
        };
    }

    // Push a new impl as an Item to the current scope
    fn push_impl_item(&mut self, impl_item: Box<Item>) {
        match &mut self.scope_stack.back_mut() {
            Some(back) => match back {
                // ScopeType::InlineMod(item_vec) |
                ScopeType::ToplevelScope(item_vec) => {
                    item_vec.push(impl_item);
                }
                // [continues]
                _ => panic!("Expected a ScopeType with an item_vec"),
            },
            None => panic!("Expected scope_stack non-empty"),
        };
    }

    // Initialize the scope_stack with the ToplevelScope
    pub fn init_scope_stack(&mut self) {
        self.scope_stack.push_back(ScopeType::ToplevelScope(ThinVec::new()));
    }

    // Push/enter a new ScopeType::FnBody
    fn enter_fn_body(&mut self) {
        self.scope_stack.push_back(ScopeType::FnBody(ThinVec::new()));
    }
    // [continues]

    // Take an if stmt and walk all blocks to locate exit ppts and insert
    // log stmts to log exit ppts.
    // * `expr` - If expression.
    // * `exit_counter` - Gives the previously seen number of exit ppts.
    // * `ppt_name` - The ppt name.
    // * `dtrace_param_blocks` - Vec of String blocks, with the ith block
    //                           giving the dtrace calls needed to log the
    //                           ith parameter.
    // * `param_to_block_idx` - Map of param identifiers to idx into
    //                          dtrace_param_blocks.
    // * `ret_ty` - Return type of the function.
    // * `daikon_tmp_counter` - Gives the number of previously allocated
    //                          temporaries added into the code.
    fn instrument_if_stmt(
        &mut self,
        expr: &mut Box<Expr>,
        exit_counter: &mut usize,
        ppt_name: &str,
        dtrace_param_blocks: &mut Vec<String>,
        param_to_block_idx: &FxHashMap<String, i32>,
        ret_ty: &FnRetTy,
        daikon_tmp_counter: &mut u32,
    ) {
        match &mut expr.kind {
            ExprKind::Block(block, _) => {
                self.instrument_block(
                    ppt_name,
                    block,
                    dtrace_param_blocks,
                    &param_to_block_idx,
                    &ret_ty,
                    exit_counter,
                    daikon_tmp_counter,
                );
            }
            ExprKind::If(_, then_block, elif_block) => {
                self.instrument_block(
                    ppt_name,
                    then_block,
                    dtrace_param_blocks,
                    &param_to_block_idx,
                    &ret_ty,
                    exit_counter,
                    daikon_tmp_counter,
                );
                match elif_block {
                    Some(elif_block) => {
                        self.instrument_if_stmt(
                            elif_block,
                            exit_counter,
                            ppt_name,
                            dtrace_param_blocks,
                            &param_to_block_idx,
                            &ret_ty,
                            daikon_tmp_counter,
                        );
                    }
                    None => {}
                }
            }
            _ => panic!("Internal error handling if stmt!"),
        }
    }

    // FIXME: noted elsewhere, but also here: implement data structures
    // to store exit ppt information rather than in dtrace_param_blocks
    // as a string. This allows for much greater flexibility, and detects
    // parse errors earlier in the dtrace pass.

    // Given a ret_expr from an explicit return stmt or a non-semi
    // trailing return, insert code into body at index i to log the
    // ret_expr.
    // * `i` - Index into block to insert logging.
    // * `ret_expr` - Expr representing the return value at a given exit ppt.
    // * `body` - The block to insert into.
    // * `exit_counter` - Unique, function-local numeric identifier for an exit ppt.
    // * `ppt_name` - Program point name.
    // * `dtrace_param_blocks` - Vec of logging code stored in Strings.
    // * `ret_ty` - Return type of the function.
    // * `daikon_tmp_counter` - Label for the next temporary variable.
    fn insert_return(
        &mut self,
        i: &mut usize,
        ret_expr: &Expr, // &Box<Expr>?
        body: &mut Box<Block>,
        exit_counter: &mut usize,
        ppt_name: &str,
        dtrace_param_blocks: &mut Vec<String>,
        ret_ty: &FnRetTy,
        daikon_tmp_counter: &mut u32,
    ) {
        // ${program_point} -> ppt_name
        // ${exit_num} -> exit_counter
        let exit = substitute(
            FxHashMap::from_iter([
                ("${program_point}", ppt_name),
                ("${exit_num}", &*exit_counter.to_string()),
            ]),
            DTRACE_EXIT,
        );
        *exit_counter += 1;

        *i = self.insert_into_block(*i, &exit, body);

        for param_block in &mut *dtrace_param_blocks {
            *i = self.insert_into_block(*i, &param_block, body);
        }

        let mut ret_is_ref = false;
        let r_ty = match &ret_ty {
            FnRetTy::Default(_span) => RustType::NoRet,
            FnRetTy::Ty(ty) => get_rust_type(&ty.kind, &mut ret_is_ref),
        };
        let pr_ty = match &ret_ty {
            FnRetTy::Ty(ty) => ty,
            _ => panic!("Inconsistent return type"),
        };
        // Process return expr
        let expr = pprust::expr_to_string(&ret_expr);
        let ret_let = substitute(
            FxHashMap::from_iter([
                ("${ret_ty}", pprust::ty_to_string(&pr_ty).as_str()),
                ("${ret_expr}", expr.as_str()),
            ]),
            DTRACE_LET_RET,
        );

        *i = self.insert_into_block(*i, &ret_let, body);
        match &r_ty {
            RustType::Prim(p_type) => {
                let prim_record_ret = if p_type == "String" || p_type == "str" {
                    String::from(DTRACE_PRIM_TOSTRING_RET)
                } else if ret_is_ref {
                    substitute(
                        FxHashMap::from_iter([("${prim_type}", p_type.as_str())]),
                        DTRACE_PRIM_REF_RET,
                    )
                } else {
                    substitute(
                        FxHashMap::from_iter([("${prim_type}", p_type.as_str())]),
                        DTRACE_PRIM_RET,
                    )
                };
                *i = self.insert_into_block(*i, &prim_record_ret, body);
            }
            RustType::UserDef(_) => {
                if ret_is_ref == false {
                    let userdef_record_ret = String::from(DTRACE_USERDEF_RET_AMPERSAND);
                    *i = self.insert_into_block(*i, &userdef_record_ret, body);
                } else {
                    let userdef_record_ret = String::from(DTRACE_USERDEF_RET);
                    *i = self.insert_into_block(*i, &userdef_record_ret, body);
                }
            }
            RustType::PrimVec(p_type) => {
                let first_tmp: &str = &daikon_tmp_counter.to_string();
                *daikon_tmp_counter += 1;
                let next_tmp = &daikon_tmp_counter.to_string();
                *daikon_tmp_counter += 1;
                let print_vec = if p_type == "String" || p_type == "str" {
                    substitute(
                        FxHashMap::from_iter([
                            ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                            ("${variable_name}", "return"),
                        ]),
                        DTRACE_PRINT_STRING_VEC,
                    )
                } else {
                    substitute(
                        FxHashMap::from_iter([
                            ("${prim_type}", p_type.as_str()),
                            ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                            ("${variable_name}", "return"),
                        ]),
                        DTRACE_PRINT_PRIM_VEC,
                    )
                };
                let prim_vec_record_ret = format!(
                    "{}\n{}\n{}",
                    substitute(
                        FxHashMap::from_iter([
                            ("${prim_type}", p_type.as_str()),
                            ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                            ("${counter_name}", &format!("__daikon_tmp{}", next_tmp)),
                            ("${variable_name}", "__daikon_ret")
                        ]),
                        DTRACE_TMP_VEC_PRIM
                    ),
                    String::from(DTRACE_VEC_POINTER_RET),
                    print_vec.clone()
                );
                *i = self.insert_into_block(*i, &prim_vec_record_ret, body);
            }
            RustType::UserDefVec(basic_type) => {
                let first_tmp = &daikon_tmp_counter.to_string();
                *daikon_tmp_counter += 1;
                let next_tmp = &daikon_tmp_counter.to_string();
                *daikon_tmp_counter += 1;
                let userdef_vec_record_ret = format!(
                    "{}\n{}\n{}\n{}",
                    substitute(
                        FxHashMap::from_iter([
                            ("${type}", basic_type.as_str()),
                            ("${counter_name}", format!("__daikon_tmp{}", next_tmp).as_str()),
                            ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                            ("${variable_name}", "__daikon_ret")
                        ]),
                        DTRACE_TMP_VEC
                    ),
                    String::from(DTRACE_VEC_POINTER_RET),
                    substitute(
                        FxHashMap::from_iter([
                            ("${type}", basic_type.as_str()),
                            ("${variable_name}", "return"),
                            ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str())
                        ]),
                        DTRACE_PRINT_POINTER_VEC
                    ),
                    substitute(
                        FxHashMap::from_iter([
                            ("${type}", basic_type.as_str()),
                            ("${variable_name}", "return"),
                            ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str())
                        ]),
                        DTRACE_VEC_FIELDS
                    )
                );
                *i = self.insert_into_block(*i, &userdef_vec_record_ret, body);
            }
            RustType::PrimArray(p_type) => {
                let first_tmp = &daikon_tmp_counter.to_string();
                *daikon_tmp_counter += 1;
                let next_tmp = &daikon_tmp_counter.to_string();
                *daikon_tmp_counter += 1;
                let print_vec = if p_type == "String" || p_type == "str" {
                    substitute(
                        FxHashMap::from_iter([
                            ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                            ("${variable_name}", "return"),
                        ]),
                        DTRACE_PRINT_STRING_VEC,
                    )
                } else {
                    substitute(
                        FxHashMap::from_iter([
                            ("${prim_type}", p_type.as_str()),
                            ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                            ("${variable_name}", "return"),
                        ]),
                        DTRACE_PRINT_PRIM_VEC,
                    )
                };
                let prim_vec_record_ret = format!(
                    "{}\n{}\n{}",
                    substitute(
                        FxHashMap::from_iter([
                            ("${prim_type}", p_type.as_str()),
                            ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                            ("${counter_name}", format!("__daikon_tmp{}", next_tmp).as_str()),
                            ("${variable_name}", "__daikon_ret")
                        ]),
                        DTRACE_TMP_VEC_PRIM
                    ),
                    String::from(DTRACE_BUILD_POINTER_ARR_RET),
                    print_vec.clone()
                );
                *i = self.insert_into_block(*i, &prim_vec_record_ret, body);
            }
            RustType::UserDefArray(basic_type) => {
                let first_tmp = &daikon_tmp_counter.to_string();
                *daikon_tmp_counter += 1;
                let next_tmp = &daikon_tmp_counter.to_string();
                *daikon_tmp_counter += 1;
                let userdef_vec_record_ret = format!(
                    "{}\n{}\n{}\n{}",
                    substitute(
                        FxHashMap::from_iter([
                            ("${type}", basic_type.as_str()),
                            ("${counter_name}", format!("__daikon_tmp{}", next_tmp).as_str()),
                            ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                            ("${variable_name}", "__daikon_ret")
                        ]),
                        DTRACE_TMP_VEC
                    ),
                    String::from(DTRACE_BUILD_POINTER_ARR_RET),
                    substitute(
                        FxHashMap::from_iter([
                            ("${type}", basic_type.as_str()),
                            ("${variable_name}", "return"),
                            ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str())
                        ]),
                        DTRACE_PRINT_POINTER_VEC
                    ),
                    substitute(
                        FxHashMap::from_iter([
                            ("${type}", basic_type.as_str()),
                            ("${variable_name}", "return"),
                            ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str())
                        ]),
                        DTRACE_VEC_FIELDS
                    )
                );
                *i = self.insert_into_block(*i, &userdef_vec_record_ret, body);
            }
            RustType::Skip => {}
            RustType::NoRet => {}
            RustType::Error => panic!("ret_ty is RustType::Error"),
        }

        *i = self.insert_into_block(*i, &String::from(DTRACE_NEWLINE), body);

        let ret = String::from(DTRACE_RET);
        *i = self.insert_into_block(*i, &ret, body);

        // remove old return stmt
        body.stmts.remove(*i);
    }

    // Given a block body and an index i, check the stmt
    // body.stmts[i] for a return stmt, a new block to walk,
    // or a stmt which invalidates one of the parameters such
    // as drop(param), and perform an appropriate action.
    // Returns the index to the next stmt in the block to process. If this
    // method adds stmts immediately after the given index, returns the next
    // stmt after all inserted stmts.
    // * `i` - Index representing the index to the stmt to process.
    // * `body` - Surrounding block containing the stmt.
    // * `exit_counter` - Int representing the next number to use to
    //                    label an exit ppt.
    // * `ppt_name` - The program point name.
    // * `dtrace_param_blocks` - Vec of String representing
    //                           instrumentation which should be
    //                           added at exit ppts.
    // * `param_to_block_idx` - No description.
    // * `ret_ty` - The return type of the function.
    fn instrument_stmt(
        &mut self,
        i: usize,
        body: &mut Box<Block>,
        exit_counter: &mut usize,
        ppt_name: &str,
        dtrace_param_blocks: &mut Vec<String>,
        param_to_block_idx: &FxHashMap<String, i32>,
        ret_ty: &FnRetTy,
        daikon_tmp_counter: &mut u32,
    ) -> usize {
        let mut block_idx = i;
        let stmt = body.stmts[block_idx].clone();
        match &mut body.stmts[block_idx].kind {
            StmtKind::Let(_local) => {
                return block_idx + 1;
            }
            StmtKind::Item(_) => {
                return block_idx + 1;
            }
            StmtKind::Expr(no_semi_expr) => match &mut no_semi_expr.kind {
                // Blocks.
                // recurse on nested block,
                // but we still only walked one (block) stmt, so just
                // move to the next stmt (return i+1)
                ExprKind::Block(block, _) => {
                    self.instrument_block(
                        ppt_name,
                        block,
                        dtrace_param_blocks,
                        &param_to_block_idx,
                        &ret_ty,
                        exit_counter,
                        daikon_tmp_counter,
                    );
                    return block_idx + 1;
                }
                ExprKind::If(_, if_block, None) => {
                    // no else
                    self.instrument_block(
                        ppt_name,
                        if_block,
                        dtrace_param_blocks,
                        &param_to_block_idx,
                        &ret_ty,
                        exit_counter,
                        daikon_tmp_counter,
                    );
                    return block_idx + 1;
                }
                ExprKind::If(_, if_block, Some(expr)) => {
                    // yes else
                    self.instrument_block(
                        ppt_name,
                        if_block,
                        dtrace_param_blocks,
                        &param_to_block_idx,
                        &ret_ty,
                        exit_counter,
                        daikon_tmp_counter,
                    );

                    self.instrument_if_stmt(
                        expr,
                        exit_counter,
                        ppt_name,
                        dtrace_param_blocks,
                        &param_to_block_idx,
                        &ret_ty,
                        daikon_tmp_counter,
                    );
                    return block_idx + 1;
                }
                ExprKind::While(_, while_block, _) => {
                    self.instrument_block(
                        ppt_name,
                        while_block,
                        dtrace_param_blocks,
                        &param_to_block_idx,
                        &ret_ty,
                        exit_counter,
                        daikon_tmp_counter,
                    );
                    return block_idx + 1;
                }
                ExprKind::ForLoop { pat: _, iter: _, body: for_block, label: _, kind: _ } => {
                    self.instrument_block(
                        ppt_name,
                        for_block,
                        dtrace_param_blocks,
                        &param_to_block_idx,
                        &ret_ty,
                        exit_counter,
                        daikon_tmp_counter,
                    );
                    return block_idx + 1;
                }
                ExprKind::Loop(loop_block, _, _) => {
                    self.instrument_block(
                        ppt_name,
                        loop_block,
                        dtrace_param_blocks,
                        &param_to_block_idx,
                        &ret_ty,
                        exit_counter,
                        daikon_tmp_counter,
                    );
                    return block_idx + 1;
                }
                // Not sure how to handle match blocks
                ExprKind::Match(_, arms, _) => {
                    for j in 0..arms.len() {
                        match &mut arms[j].body {
                            None => {}
                            Some(bd) => match &mut bd.kind {
                                ExprKind::Block(_block, _) => {
                                    // FIXME: remove this commented code.
                                    // self.instrument_block(ppt_name,
                                    //                 block,
                                    //                 dtrace_param_blocks,
                                    //                 &param_to_block_idx,
                                    //                 &ret_ty,
                                    //                 exit_counter,
                                    //                 daikon_tmp_counter);
                                }
                                _ => {} // FIXME: more careful analysis on whether this is supposed to be a return expr or not, e.g. println/panic vs 7.
                            },
                        }
                    }
                    return block_idx + 1;
                } // TryBlock, Const block? probably more
                _ => {}
            },
            _ => {}
        }
        // Next, look for return stmts where we don't need to borrow
        // the stmt mutably. We will just mutate the block appropriately
        // and delete the original return statement.
        match &stmt.kind {
            StmtKind::Semi(semi) => match &semi.kind {
                ExprKind::Ret(None) => {
                    // ${program_point} -> ppt_name
                    // ${exit_num} -> exit_counter
                    let exit = substitute(
                        FxHashMap::from_iter([
                            ("${program_point}", ppt_name),
                            ("${exit_num}", &*exit_counter.to_string()),
                        ]),
                        DTRACE_EXIT,
                    );

                    *exit_counter += 1;
                    block_idx = self.insert_into_block(block_idx, &exit, body);
                    for param_block in &mut *dtrace_param_blocks {
                        // DAIKON TMP ERROR: you will end up using the same __daikon_tmpX values,
                        // but Rust doesn't care. Not high-priority, just weird to see
                        // let __daikon_tmp7 = ... twice in the same scope.
                        block_idx = self.insert_into_block(block_idx, &param_block, body);
                    }

                    block_idx =
                        self.insert_into_block(block_idx, &String::from(DTRACE_NEWLINE), body);

                    // we're sitting on the void return we just processed, so inc
                    // to move on.
                    block_idx += 1;
                }
                ExprKind::Ret(Some(return_expr)) => {
                    self.insert_return(
                        &mut block_idx,
                        &return_expr,
                        body,
                        exit_counter,
                        ppt_name,
                        dtrace_param_blocks,
                        ret_ty,
                        daikon_tmp_counter,
                    );
                }
                ExprKind::Call(_call, _params) => {
                    return block_idx + 1;
                } // Maybe check for drop and other invalidations.
                _ => {
                    return block_idx + 1;
                } // other things you overlooked?
            },
            // Now, any stmt without a semicolon must be a trailing return?
            // Blocks are no-semi exprs, but we should have caught them in the
            // previous match block.
            StmtKind::Expr(no_semi_expr) => {
                // we know it is not a block, so it must be trailing no-semi return expr
                self.insert_return(
                    &mut block_idx,
                    &no_semi_expr,
                    body,
                    exit_counter,
                    ppt_name,
                    dtrace_param_blocks,
                    ret_ty,
                    daikon_tmp_counter,
                );
            }
            _ => {
                return block_idx + 1;
            }
        }
        block_idx
    }

    // Get 'impl X { }' as an Item struct.
    // This will be transformed into a new impl with dtrace routines.
    fn base_impl_item(&mut self) -> Box<Item> {
        let base_impl = String::from(DTRACE_BUILD_AN_IMPL_BLOCK);
        let base_impl_item = self.parse_items_from_source_string(base_impl);
        match &base_impl_item {
            Err(_why) => panic!("Parsing base impl failed"),
            Ok(base_impl_item) => base_impl_item[0].clone(),
        }
    }

    // This function generates a new impl for a user-defined struct with type
    // ty and enqueues the impl block into self.mod_items to be appended to
    // the end of the current translation unit (file). The impl will contain
    // multiple synthesized functions, like dtrace_print_fields,
    // dtrace_print_fields_vec, and more.
    // * `struct_fields` - Fields of the struct whose impl we are generating.
    // * `struct_ty` - Type struct representing the actual type in Rust of the
    //                 struct whose impl is being generated.
    // * `struct_generics` - Required generic arguments to the struct whose
    //                       impl we are generating.
    fn gen_impl(
        &mut self,
        struct_fields: &mut ThinVec<FieldDef>,
        struct_ty: &Ty,
        struct_generics: &Generics,
    ) -> Box<Item> {
        // get the base_impl. If we are not toplevel, we should get
        // the impl as nested in a function, otherwise get the impl
        // just as an Item for the toplevel
        let mut impl_item = self.base_impl_item();
        let the_impl = match &mut impl_item.kind {
            ItemKind::Impl(i) => i,
            _ => panic!("Base impl is not impl"),
        };
        // FIXME: remove this.
        // let spliced_struct = splice_struct(&pp_struct);
        // let struct_as_ret = build_phony_ret(spliced_struct.clone()); // FIXME: fix splice string to handle pub keyword
        the_impl.self_ty = Box::new(struct_ty.clone());
        // FIXME: remove this.
        // match &self.parse_items_from_source_string(struct_as_ret) {
        //     Err(_why) => panic!("Parsing phony arg failed"),
        //     Ok(arg_items) => match &arg_items[0].kind {
        //         ItemKind::Fn(phony) => match &phony.sig.decl.output {
        //             FnRetTy::Ty(ty) => ty.clone(),
        //             _ => panic!("Phony ret is none")
        //         }
        //         _ => panic!("Parsing phony fn failed")
        //     }
        // };
        the_impl.generics = struct_generics.clone();

        let dtrace_print_fields_fn = self.build_dtrace_print_fields(struct_fields);
        match &self.parse_items_from_source_string(dtrace_print_fields_fn) {
            Err(_why) => panic!("Parsing dtrace_print_fields failed"),
            Ok(items) => match &items[0].kind {
                ItemKind::Impl(tmp_impl) => {
                    the_impl.items.push(tmp_impl.items[0].clone());
                }
                _ => panic!("Expected phony impl 1"),
            },
        }

        let plain_struct = match &struct_ty.kind {
            TyKind::Path(_, path) => String::from(path.segments[0].ident.as_str()),
            _ => panic!("Why don't we have a path?"),
        };
        let dtrace_print_fields_vec =
            self.build_dtrace_print_fields_vec(plain_struct.clone(), struct_fields);
        match &self.parse_items_from_source_string(dtrace_print_fields_vec) {
            Err(_) => panic!("Parsing dtrace_print_fields_vec failed"),
            Ok(items) => match &items[0].kind {
                ItemKind::Impl(tmp_impl) => {
                    the_impl.items.push(tmp_impl.items[0].clone());
                }
                _ => panic!("Expected phony impl 2"),
            },
        }

        // FIXME: remove this.
        // build dtrace_print_xfield_vec (AND dtrace_print_xfield...) here, then that should be it for generating fns in the impl.
        let dtrace_print_xfields =
            self.build_dtrace_print_xfield_vec(plain_struct.clone(), struct_fields);
        match &self.parse_items_from_source_string(dtrace_print_xfields) {
            Err(_) => panic!("Parsing dtrace_print_xfields failed"),
            Ok(items) => match &items[0].kind {
                ItemKind::Impl(tmp_impl) => {
                    for i in 0..tmp_impl.items.len() {
                        the_impl.items.push(tmp_impl.items[i].clone());
                    }
                }
                _ => panic!("Expected phony impl 3"),
            },
        }

        return impl_item;
    }

    // Given a struct with fields 'fields', returns a String with code
    // containing a function to log each field of the struct given a vec of
    // such a struct. String is sufficient, since no further modifications
    // or mutations will be done to this generated code.
    // Additionally, for any Vec or array fields, adds a function
    // which is responsible for logging the field in pointer format.
    // FIXME: write a small example input/output.
    // * `plain_struct` - The identifier for the struct being processed.
    // * `fields` - Data structures representing the fields of the struct
    //              being processed.
    fn build_dtrace_print_xfield_vec(
        &mut self,
        plain_struct: String,
        fields: &ThinVec<FieldDef>,
    ) -> String {
        // WARNING: also building dtrace_print_xfield here... be careful about different issues.
        // dtrace_print_xfield_vec prints scalar fields out of a Vec<Me>, and dtrace_print_xfield takes one Me and prints Me.f which is a Vec.
        let mut dtrace_print_xfields_vec = String::from(DTRACE_PRINT_XFIELDS_VEC_PROLOGUE);

        // Not important for this to be here, name clashes are not a concern since
        // this function is synthesized by the dtrace pass.
        let mut daikon_tmp_counter = 0;
        for i in 0..fields.len() {
            let field_name = match &fields[i].ident {
                Some(field_ident) => String::from(field_ident.as_str()),
                None => panic!("Field has no identifier"),
            };

            let mut is_ref = false;
            let dtrace_print_xfield = match &get_rust_type(&fields[i].ty.kind, &mut is_ref) {
                RustType::Prim(p_type) => {
                    // We have a vec of ourselves, and the field is
                    if p_type == "String" || p_type == "str" {
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", plain_struct.as_str()),
                                ("${field_name}", field_name.as_str()),
                                ("${file_name}", &*OUTPUT_PREFIX.lock().unwrap()),
                            ]),
                            DTRACE_PRINT_XFIELDS_STRING,
                        )
                    } else {
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", plain_struct.as_str()),
                                ("${field_name}", field_name.as_str()),
                                ("${file_name}", &*OUTPUT_PREFIX.lock().unwrap()),
                            ]),
                            DTRACE_PRINT_XFIELDS,
                        )
                    }
                }
                RustType::PrimVec(p_type) => {
                    let first_tmp: &str = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    let next_tmp = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    let print_vec = if p_type == "String" || p_type == "str" {
                        substitute(
                            FxHashMap::from_iter([
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                ("${field_name}", field_name.as_str()),
                            ]),
                            DTRACE_PRINT_STRING_VEC_FOR_FIELD,
                        )
                    } else {
                        substitute(
                            FxHashMap::from_iter([
                                ("${prim_type}", p_type.as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                ("${field_name}", field_name.as_str()),
                            ]),
                            DTRACE_PRINT_PRIM_VEC_FOR_FIELD,
                        )
                    };
                    let f1 = format!(
                        "{}\n{}\n{}\n{}\n{}\n{}",
                        substitute(
                            FxHashMap::from_iter([("${field_name}", field_name.as_str())]),
                            DTRACE_PRINT_XFIELD_FOR_FIELD_PROLOGUE
                        ),
                        substitute(
                            FxHashMap::from_iter([
                                ("${prim_type}", p_type.as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                ("${counter_name}", format!("__daikon_tmp{}", next_tmp).as_str()),
                                ("${field_name}", field_name.as_str())
                            ]),
                            DTRACE_TMP_PRIM_VEC_FOR_FIELD
                        ),
                        substitute(
                            FxHashMap::from_iter([("${field_name}", field_name.as_str())]),
                            DTRACE_POINTER_VEC_USERDEF
                        ),
                        String::from(DTRACE_PRINT_XFIELD_FOR_FIELD_MID),
                        print_vec.clone(),
                        String::from(DTRACE_PRINT_XFIELD_FOR_FIELD_EPILOGUE)
                    );
                    let f2 = substitute(
                        FxHashMap::from_iter([
                            ("${type}", plain_struct.as_str()),
                            ("${field_name}", field_name.as_str()),
                            ("${file_name}", &*OUTPUT_PREFIX.lock().unwrap()),
                        ]),
                        DTRACE_PRINT_XFIELDS_VEC,
                    );
                    format!("{}\n{}", f1, f2)
                }
                RustType::UserDefVec(basic_struct) => {
                    let first_tmp: &str = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    let next_tmp = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    // We maintain that is_ref represents Vec/array args in this case.
                    // NOTE: are these swapped?
                    let tmp_vec = if is_ref {
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", basic_struct.as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                ("${counter_name}", format!("__daikon_tmp{}", next_tmp).as_str()),
                                ("${field_name}", field_name.as_str()),
                            ]),
                            DTRACE_TMP_VEC_FOR_FIELD,
                        )
                    } else {
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", basic_struct.as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                ("${counter_name}", format!("__daikon_tmp{}", next_tmp).as_str()),
                                ("${field_name}", field_name.as_str()),
                            ]),
                            DTRACE_TMP_VEC_FOR_FIELD_AMPERSAND,
                        )
                    };
                    let f1 = format!(
                        "{}\n{}\n{}\n{}\n{}\n{}\n{}",
                        substitute(
                            FxHashMap::from_iter([("${field_name}", field_name.as_str())]),
                            DTRACE_PRINT_XFIELD_FOR_FIELD_PROLOGUE
                        ),
                        tmp_vec.clone(),
                        substitute(
                            FxHashMap::from_iter([("${field_name}", field_name.as_str())]),
                            DTRACE_POINTER_VEC_USERDEF
                        ),
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", basic_struct.as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                ("${field_name}", field_name.as_str())
                            ]),
                            DTRACE_POINTERS_VEC_USERDEF
                        ),
                        String::from(DTRACE_PRINT_XFIELD_FOR_FIELD_MID),
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", basic_struct.as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                ("${field_name}", field_name.as_str())
                            ]),
                            DTRACE_PRINT_FIELDS_FOR_FIELD
                        ),
                        String::from(DTRACE_PRINT_XFIELD_FOR_FIELD_EPILOGUE)
                    );
                    let f2 = substitute(
                        FxHashMap::from_iter([
                            ("${type}", plain_struct.as_str()),
                            ("${field_name}", field_name.as_str()),
                            ("${file_name}", &*OUTPUT_PREFIX.lock().unwrap()),
                        ]),
                        DTRACE_PRINT_XFIELDS_VEC,
                    );
                    format!("{}\n{}", f1, f2)
                }
                // FIXME: arrays, mighty similar to vec. Maybe you can cheat and just do the exact same thing... use | in pattern matching.
                // Except pointer is diff, as_ptr() as usize vs as *const _ as *const () as usize...
                RustType::PrimArray(p_type) => {
                    // UNTRUSTED:
                    let first_tmp: &str = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    let next_tmp = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    let print_vec = if p_type == "String" || p_type == "str" {
                        substitute(
                            FxHashMap::from_iter([
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                ("${field_name}", field_name.as_str()),
                            ]),
                            DTRACE_PRINT_STRING_VEC_FOR_FIELD,
                        )
                    } else {
                        substitute(
                            FxHashMap::from_iter([
                                ("${prim_type}", p_type.as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                ("${field_name}", field_name.as_str()),
                            ]),
                            DTRACE_PRINT_PRIM_VEC_FOR_FIELD,
                        )
                    };
                    let f1 = format!(
                        "{}\n{}\n{}\n{}\n{}\n{}",
                        substitute(
                            FxHashMap::from_iter([("${field_name}", field_name.as_str())]),
                            DTRACE_PRINT_XFIELD_FOR_FIELD_PROLOGUE
                        ),
                        substitute(
                            FxHashMap::from_iter([
                                ("${prim_type}", p_type.as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                ("${counter_name}", format!("__daikon_tmp{}", next_tmp).as_str()),
                                ("${field_name}", field_name.as_str())
                            ]),
                            DTRACE_TMP_PRIM_VEC_FOR_FIELD
                        ),
                        substitute(
                            FxHashMap::from_iter([("${field_name}", field_name.as_str())]),
                            DTRACE_POINTER_ARR_USERDEF
                        ),
                        String::from(DTRACE_PRINT_XFIELD_FOR_FIELD_MID),
                        print_vec.clone(),
                        String::from(DTRACE_PRINT_XFIELD_FOR_FIELD_EPILOGUE)
                    );
                    let f2 = substitute(
                        FxHashMap::from_iter([
                            ("${type}", plain_struct.as_str()),
                            ("${field_name}", field_name.as_str()),
                            ("${file_name}", &*OUTPUT_PREFIX.lock().unwrap()),
                        ]),
                        DTRACE_PRINT_XFIELDS_VEC,
                    );
                    format!("{}\n{}", f1, f2)
                }
                RustType::UserDefArray(basic_struct) => {
                    // UNTRUSTED:
                    let first_tmp: &str = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    let next_tmp = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    // We maintain that is_ref represents Vec/array args in this case.
                    // NOTE: are these swapped?
                    let tmp_vec = if is_ref {
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", basic_struct.as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                ("${counter_name}", format!("__daikon_tmp{}", next_tmp).as_str()),
                                ("${field_name}", field_name.as_str()),
                            ]),
                            DTRACE_TMP_VEC_FOR_FIELD,
                        )
                    } else {
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", basic_struct.as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                ("${counter_name}", format!("__daikon_tmp{}", next_tmp).as_str()),
                                ("${field_name}", field_name.as_str()),
                            ]),
                            DTRACE_TMP_VEC_FOR_FIELD_AMPERSAND,
                        )
                    };
                    let f1 = format!(
                        "{}\n{}\n{}\n{}\n{}\n{}\n{}",
                        substitute(
                            FxHashMap::from_iter([("${field_name}", field_name.as_str())]),
                            DTRACE_PRINT_XFIELD_FOR_FIELD_PROLOGUE
                        ),
                        tmp_vec.clone(),
                        substitute(
                            FxHashMap::from_iter([("${field_name}", field_name.as_str())]),
                            DTRACE_POINTER_ARR_USERDEF
                        ),
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", basic_struct.as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                ("${field_name}", field_name.as_str())
                            ]),
                            DTRACE_POINTERS_VEC_USERDEF
                        ),
                        String::from(DTRACE_PRINT_XFIELD_FOR_FIELD_MID),
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", basic_struct.as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                ("${field_name}", field_name.as_str())
                            ]),
                            DTRACE_PRINT_FIELDS_FOR_FIELD
                        ),
                        String::from(DTRACE_PRINT_XFIELD_FOR_FIELD_EPILOGUE)
                    );
                    let f2 = substitute(
                        FxHashMap::from_iter([
                            ("${type}", plain_struct.as_str()),
                            ("${field_name}", field_name.as_str()),
                            ("${file_name}", &*OUTPUT_PREFIX.lock().unwrap()),
                        ]),
                        DTRACE_PRINT_XFIELDS_VEC,
                    );
                    format!("{}\n{}", f1, f2)
                }
                _ => String::from(""),
            };

            dtrace_print_xfields_vec.push_str(&dtrace_print_xfield);
        }
        let res = format!(
            "{}{}",
            dtrace_print_xfields_vec,
            String::from(DTRACE_PRINT_XFIELDS_VEC_EPILOGUE)
        );
        res
    }

    // Builds the top-level function which is called to log a Vec or array of a given struct.
    // * `plain_struct` - Identifier for the struct being processed.
    // * `fields` - Data structures representing the fields of the struct being processed.
    fn build_dtrace_print_fields_vec(
        &mut self,
        plain_struct: String,
        fields: &ThinVec<FieldDef>,
    ) -> String {
        let mut dtrace_print_fields_vec = substitute(
            FxHashMap::from_iter([("${type}", plain_struct.as_str())]),
            DTRACE_PRINT_FIELDS_VEC_PROLOGUE,
        );

        let mut daikon_tmp_counter = 0;
        for i in 0..fields.len() {
            let field_name = match &fields[i].ident {
                Some(field_ident) => String::from(field_ident.as_str()),
                None => panic!("Field has no identifier"),
            };

            let mut is_ref = false;
            let dtrace_field_vec_rec = match &get_rust_type(&fields[i].ty.kind, &mut is_ref) {
                // don't need p_type because we just call dtrace_print_xfield which handles the type.
                RustType::Prim(_) => {
                    let first_tmp: &str = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    let next_tmp = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    format!(
                        "{}\n{}",
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", plain_struct.as_str()),
                                ("${counter_name}", format!("__daikon_tmp{}", next_tmp).as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                            ]),
                            DTRACE_TMP_VEC_USERDEF
                        ),
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", plain_struct.as_str()),
                                ("${field_name}", field_name.as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str())
                            ]),
                            DTRACE_PRINT_XFIELD_VEC
                        )
                    )
                }
                RustType::UserDef(field_type) => {
                    let first_tmp: &str = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    let next_tmp = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    let tmp_vec = if !is_ref {
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", field_type.as_str()),
                                ("${counter_name}", format!("__daikon_tmp{}", next_tmp).as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                ("${field_name}", field_name.as_str()),
                            ]),
                            DTRACE_TMP_VEC_USERDEF_FIELD_AMPERSAND,
                        )
                    } else {
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", field_type.as_str()),
                                ("${counter_name}", format!("__daikon_tmp{}", next_tmp).as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                ("${field_name}", field_name.as_str()),
                            ]),
                            DTRACE_TMP_VEC_USERDEF_FIELD,
                        )
                    };
                    format!(
                        "{}\n{}\n{}",
                        tmp_vec,
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", field_type.as_str()),
                                ("${field_name}", field_name.as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str())
                            ]),
                            DTRACE_PRINT_POINTER_VEC_USERDEF
                        ),
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", field_type.as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                ("${field_name}", field_name.as_str())
                            ]),
                            DTRACE_USERDEF_VEC_FIELDS
                        )
                    )
                }
                // call X::dtrace_print_<field>_vec since it will be implemented to only print pointers. NOT TRUSTED CODE:
                RustType::PrimVec(_) => {
                    let first_tmp: &str = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    let next_tmp = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    format!(
                        "{}\n{}",
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", plain_struct.as_str()),
                                ("${counter_name}", format!("__daikon_tmp{}", next_tmp).as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                            ]),
                            DTRACE_TMP_VEC_USERDEF
                        ),
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", plain_struct.as_str()),
                                ("${field_name}", field_name.as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str())
                            ]),
                            DTRACE_PRINT_XFIELD_VEC
                        )
                    )
                }
                RustType::UserDefVec(_) => {
                    let first_tmp: &str = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    let next_tmp = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    format!(
                        "{}\n{}",
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", plain_struct.as_str()),
                                ("${counter_name}", format!("__daikon_tmp{}", next_tmp).as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                            ]),
                            DTRACE_TMP_VEC_USERDEF
                        ),
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", plain_struct.as_str()),
                                ("${field_name}", field_name.as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str())
                            ]),
                            DTRACE_PRINT_XFIELD_VEC
                        )
                    )
                }
                RustType::PrimArray(_) => {
                    // UNTRUSTED: is this exactly the same?
                    let first_tmp: &str = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    let next_tmp = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    format!(
                        "{}\n{}",
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", plain_struct.as_str()),
                                ("${counter_name}", format!("__daikon_tmp{}", next_tmp).as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                            ]),
                            DTRACE_TMP_VEC_USERDEF
                        ),
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", plain_struct.as_str()),
                                ("${field_name}", field_name.as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str())
                            ]),
                            DTRACE_PRINT_XFIELD_VEC
                        )
                    )
                }
                RustType::UserDefArray(_) => {
                    // UNTRUSTED: is this exactly the same?
                    let first_tmp: &str = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    let next_tmp = &daikon_tmp_counter.to_string();
                    daikon_tmp_counter += 1;
                    format!(
                        "{}\n{}",
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", plain_struct.as_str()),
                                ("${counter_name}", format!("__daikon_tmp{}", next_tmp).as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                            ]),
                            DTRACE_TMP_VEC_USERDEF
                        ),
                        substitute(
                            FxHashMap::from_iter([
                                ("${type}", plain_struct.as_str()),
                                ("${field_name}", field_name.as_str()),
                                ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str())
                            ]),
                            DTRACE_PRINT_XFIELD_VEC
                        )
                    )
                }
                RustType::Skip => {
                    continue;
                }
                RustType::NoRet => String::from(""),
                RustType::Error => panic!("Field type not handled"),
            };

            dtrace_print_fields_vec.push_str(&dtrace_field_vec_rec); // don't think a newline here would matter? Parsing doesn't care.
        }

        let res = format!(
            "{}{}",
            dtrace_print_fields_vec,
            String::from(DTRACE_PRINT_FIELDS_VEC_EPILOGUE)
        );
        res
    }

    // Given a struct's field declarations, generate the function dtrace_print_fields(self)
    // to be added to the synthesized impl block.
    // * `fields` - Data structures representing the fields of the struct being processed.
    fn build_dtrace_print_fields(&mut self, fields: &mut ThinVec<FieldDef>) -> String {
        let mut dtrace_print_fields: String = String::from(DTRACE_PRINT_FIELDS_PROLOGUE);

        for i in 0..fields.len() {
            let field_name = match &fields[i].ident {
                Some(field_ident) => String::from(field_ident.as_str()),
                None => panic!("Field has no identifier"),
            };

            let mut is_ref = false;
            let mut dtrace_field_rec = match &get_rust_type(&fields[i].ty.kind, &mut is_ref) {
                RustType::Prim(p_type) => {
                    if p_type == "String" || p_type == "str" {
                        substitute(
                            FxHashMap::from_iter([("${field_name}", field_name.as_str())]),
                            DTRACE_PRIM_FIELD_TOSTRING,
                        )
                    } else if is_ref {
                        substitute(
                            FxHashMap::from_iter([
                                ("${prim_type}", p_type.as_str()),
                                ("${field_name}", field_name.as_str()),
                            ]),
                            DTRACE_PRIM_REF_STRUCT,
                        )
                    } else {
                        substitute(
                            FxHashMap::from_iter([
                                ("${prim_type}", p_type.as_str()),
                                ("${field_name}", field_name.as_str()),
                            ]),
                            DTRACE_PRIM_STRUCT,
                        )
                    }
                }
                RustType::UserDef(_) => {
                    if !is_ref {
                        substitute(
                            FxHashMap::from_iter([("${field_name}", field_name.as_str())]),
                            DTRACE_USERDEF_STRUCT_AMPERSAND,
                        )
                    } else {
                        substitute(
                            FxHashMap::from_iter([("${field_name}", field_name.as_str())]),
                            DTRACE_USERDEF_STRUCT,
                        )
                    }
                }
                RustType::PrimVec(_)
                | RustType::UserDefVec(_)
                | RustType::PrimArray(_)
                | RustType::UserDefArray(_) => substitute(
                    FxHashMap::from_iter([("${field_name}", field_name.as_str())]),
                    DTRACE_CALL_PRINT_FIELD,
                ),
                RustType::Skip => {
                    continue;
                }
                RustType::NoRet => String::from(""),
                RustType::Error => panic!("Field type not handled"),
            };
            dtrace_field_rec.push_str("\n");

            dtrace_print_fields.push_str(&format!("{}{}", dtrace_field_rec, "\n"));
        }

        format!("{}{}", dtrace_print_fields, String::from(DTRACE_PRINT_FIELDS_EPILOGUE))
    }

    // FIXME: dtrace calls should be represented with a better data structures rather than
    // Strings.
    // Given a function signature, generate a dtrace call for each parameter including
    // necessary setup library calls,
    // These will be used at the function entry and each exit ppt, potentially updating
    // names of synthesized/injected variables to avoid name conflicts (this should become
    // easy with better data structures generated by this function).
    // * `decl` - Function declaration of the function being processed.
    // * `daikon_tmp_counter` - Stores the number of temporary variables used in the
    //                          instrumentation code.
    fn fn_sig_to_dtrace_code(
        &mut self,
        decl: &Box<FnDecl>,
        daikon_tmp_counter: &mut u32,
    ) -> Vec<String> {
        // Process params.
        let mut dtrace_param_blocks: Vec<String> = Vec::new();
        for i in 0..decl.inputs.len() {
            let mut is_ref = false;
            let var_name = get_param_ident(&decl.inputs[i].pat);
            let mut dtrace_rec = if get_param_ident(&decl.inputs[i].pat) == "self" {
                substitute(
                    FxHashMap::from_iter([("${variable_name}", var_name.as_str())]),
                    DTRACE_USERDEF,
                )
            } else {
                match &get_rust_type(&decl.inputs[i].ty.kind, &mut is_ref) {
                    RustType::Prim(p_type) => {
                        if p_type == "String" || p_type == "str" {
                            substitute(
                                FxHashMap::from_iter([(
                                    "${variable_name}",
                                    get_param_ident(&decl.inputs[i].pat).as_str(),
                                )]),
                                DTRACE_PRIM_TOSTRING,
                            )
                        } else if is_ref {
                            substitute(
                                FxHashMap::from_iter([
                                    ("${prim_type}", p_type.as_str()),
                                    (
                                        "${variable_name}",
                                        get_param_ident(&decl.inputs[i].pat).as_str(),
                                    ),
                                ]),
                                DTRACE_PRIM_REF,
                            )
                        } else {
                            substitute(
                                FxHashMap::from_iter([
                                    ("${prim_type}", p_type.as_str()),
                                    (
                                        "${variable_name}",
                                        get_param_ident(&decl.inputs[i].pat).as_str(),
                                    ),
                                ]),
                                DTRACE_PRIM,
                            )
                        }
                    }
                    RustType::UserDef(_) => {
                        if !is_ref {
                            substitute(
                                FxHashMap::from_iter([("${variable_name}", var_name.as_str())]),
                                DTRACE_USERDEF_AMPERSAND,
                            )
                        } else {
                            substitute(
                                FxHashMap::from_iter([("${variable_name}", var_name.as_str())]),
                                DTRACE_USERDEF,
                            )
                        }
                    }
                    RustType::PrimVec(p_type) => {
                        let first_tmp: &str = &daikon_tmp_counter.to_string();
                        *daikon_tmp_counter += 1;
                        let next_tmp = &daikon_tmp_counter.to_string();
                        *daikon_tmp_counter += 1;
                        let print_vec = if p_type == "String" || p_type == "str" {
                            substitute(
                                FxHashMap::from_iter([
                                    ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                    (
                                        "${variable_name}",
                                        get_param_ident(&decl.inputs[i].pat).as_str(),
                                    ),
                                ]),
                                DTRACE_PRINT_STRING_VEC,
                            )
                        } else {
                            substitute(
                                FxHashMap::from_iter([
                                    ("${prim_type}", p_type.as_str()),
                                    ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                    ("${variable_name}", &get_param_ident(&decl.inputs[i].pat)),
                                ]),
                                DTRACE_PRINT_PRIM_VEC,
                            )
                        };
                        format!(
                            "{}\n{}\n{}",
                            substitute(
                                FxHashMap::from_iter([
                                    ("${prim_type}", p_type.as_str()),
                                    ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                    (
                                        "${counter_name}",
                                        format!("__daikon_tmp{}", next_tmp).as_str()
                                    ),
                                    (
                                        "${variable_name}",
                                        get_param_ident(&decl.inputs[i].pat).as_str()
                                    )
                                ]),
                                DTRACE_TMP_VEC_PRIM
                            ),
                            substitute(
                                FxHashMap::from_iter([(
                                    "${variable_name}",
                                    get_param_ident(&decl.inputs[i].pat).as_str()
                                )]),
                                DTRACE_VEC_POINTER
                            ),
                            print_vec.clone()
                        )
                    }
                    RustType::UserDefVec(basic_type) => {
                        let first_tmp = &daikon_tmp_counter.to_string();
                        *daikon_tmp_counter += 1;
                        let next_tmp = &daikon_tmp_counter.to_string();
                        *daikon_tmp_counter += 1;
                        let var_name = get_param_ident(&decl.inputs[i].pat);
                        // We maintain that is_ref represents Vec/array argument in this case.
                        let tmp_vec = if is_ref {
                            substitute(
                                FxHashMap::from_iter([
                                    ("${type}", basic_type.as_str()),
                                    (
                                        "${counter_name}",
                                        format!("__daikon_tmp{}", next_tmp).as_str(),
                                    ),
                                    ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                    ("${variable_name}", var_name.as_str()),
                                ]),
                                DTRACE_TMP_VEC,
                            )
                        } else {
                            substitute(
                                FxHashMap::from_iter([
                                    ("${type}", basic_type.as_str()),
                                    ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                    (
                                        "${counter_name}",
                                        format!("__daikon_tmp{}", next_tmp).as_str(),
                                    ),
                                    ("${variable_name}", var_name.as_str()),
                                ]),
                                DTRACE_TMP_VEC_AMPERSAND,
                            )
                        };
                        let res = format!(
                            "{}\n{}\n{}\n{}",
                            tmp_vec.clone(),
                            substitute(
                                FxHashMap::from_iter([(
                                    "${variable_name}",
                                    get_param_ident(&decl.inputs[i].pat).as_str()
                                )]),
                                DTRACE_VEC_POINTER
                            ),
                            substitute(
                                FxHashMap::from_iter([
                                    ("${type}", basic_type.as_str()),
                                    ("${variable_name}", var_name.as_str()),
                                    ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str())
                                ]),
                                DTRACE_PRINT_POINTER_VEC
                            ),
                            substitute(
                                FxHashMap::from_iter([
                                    ("${type}", basic_type.as_str()),
                                    ("${variable_name}", var_name.as_str()),
                                    ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str())
                                ]),
                                DTRACE_VEC_FIELDS
                            )
                        );
                        res
                    }
                    RustType::PrimArray(p_type) => {
                        let first_tmp: &str = &daikon_tmp_counter.to_string();
                        *daikon_tmp_counter += 1;
                        let next_tmp = &daikon_tmp_counter.to_string();
                        *daikon_tmp_counter += 1;
                        let print_vec = if p_type == "String" || p_type == "str" {
                            substitute(
                                FxHashMap::from_iter([
                                    ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                    (
                                        "${variable_name}",
                                        get_param_ident(&decl.inputs[i].pat).as_str(),
                                    ),
                                ]),
                                DTRACE_PRINT_STRING_VEC,
                            )
                        } else {
                            substitute(
                                FxHashMap::from_iter([
                                    ("${prim_type}", p_type.as_str()),
                                    ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                    (
                                        "${variable_name}",
                                        get_param_ident(&decl.inputs[i].pat).as_str(),
                                    ),
                                ]),
                                DTRACE_PRINT_PRIM_VEC,
                            )
                        };
                        format!(
                            "{}\n{}\n{}",
                            substitute(
                                FxHashMap::from_iter([
                                    ("${prim_type}", p_type.as_str()),
                                    ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                    (
                                        "${counter_name}",
                                        format!("__daikon_tmp{}", next_tmp).as_str()
                                    ),
                                    (
                                        "${variable_name}",
                                        get_param_ident(&decl.inputs[i].pat).as_str()
                                    )
                                ]),
                                DTRACE_TMP_VEC_PRIM
                            ),
                            substitute(
                                FxHashMap::from_iter([(
                                    "${variable_name}",
                                    get_param_ident(&decl.inputs[i].pat).as_str()
                                )]),
                                DTRACE_BUILD_POINTER_ARR
                            ),
                            print_vec.clone()
                        )
                    }
                    RustType::UserDefArray(basic_type) => {
                        let first_tmp = &daikon_tmp_counter.to_string();
                        *daikon_tmp_counter += 1;
                        let next_tmp = &daikon_tmp_counter.to_string();
                        *daikon_tmp_counter += 1;
                        let var_name = get_param_ident(&decl.inputs[i].pat);
                        // We maintain that is_ref represents Vec/array argument in this case.
                        let tmp_vec = if is_ref {
                            substitute(
                                FxHashMap::from_iter([
                                    ("${type}", basic_type.as_str()),
                                    (
                                        "${counter_name}",
                                        format!("__daikon_tmp{}", next_tmp).as_str(),
                                    ),
                                    ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                    ("${variable_name}", var_name.as_str()),
                                ]),
                                DTRACE_TMP_VEC,
                            )
                        } else {
                            substitute(
                                FxHashMap::from_iter([
                                    ("${type}", basic_type.as_str()),
                                    ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str()),
                                    (
                                        "${counter_name}",
                                        format!("__daikon_tmp{}", next_tmp).as_str(),
                                    ),
                                    ("${variable_name}", var_name.as_str()),
                                ]),
                                DTRACE_TMP_VEC_AMPERSAND,
                            )
                        };
                        let res = format!(
                            "{}\n{}\n{}\n{}",
                            tmp_vec.clone(),
                            substitute(
                                FxHashMap::from_iter([("${variable_name}", var_name.as_str())]),
                                DTRACE_BUILD_POINTER_ARR
                            ),
                            substitute(
                                FxHashMap::from_iter([
                                    ("${type}", basic_type.as_str()),
                                    ("${variable_name}", var_name.as_str()),
                                    ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str())
                                ]),
                                DTRACE_PRINT_POINTER_VEC
                            ),
                            substitute(
                                FxHashMap::from_iter([
                                    ("${type}", basic_type.as_str()),
                                    ("${variable_name}", var_name.as_str()),
                                    ("${vec_name}", format!("__daikon_tmp{}", first_tmp).as_str())
                                ]),
                                DTRACE_VEC_FIELDS
                            )
                        );
                        res
                    }
                    RustType::Skip => {
                        continue;
                    }
                    RustType::NoRet => String::from(""),
                    RustType::Error => panic!("Formal arg type not handled."),
                }
            };
            dtrace_rec.push_str("\n");

            dtrace_param_blocks.push(format!("{}{}", dtrace_rec, "\n"));
        }

        // Return param-dependent dtrace calls.
        dtrace_param_blocks
    }

    // Visit a single block, used for recursing through nested
    // blocks like if stmts and loops.
    // * `ppt_name` - Program point name.
    // * `body` - The block to instrument.
    // * `dtrace_param_blocks` - Vec of Strings containing dtrace
    //                           calls for each parameter.
    // * `param_to_block_idx` - No description.
    // * `ret_ty` - Function return type.
    // * `exit_counter` - Contains the next label for an exit ppt.
    // * `daikon_tmp_counter` - Contains the next label for a temporary
    //                          variable used in the instrumented code.
    fn instrument_block(
        &mut self,
        ppt_name: &str,
        body: &mut Box<Block>,
        dtrace_param_blocks: &mut Vec<String>,
        param_to_block_idx: &FxHashMap<String, i32>,
        ret_ty: &FnRetTy,
        exit_counter: &mut usize,
        daikon_tmp_counter: &mut u32,
    ) {
        let mut i = 0;

        // Assuming no unreachable statements.
        while i < body.stmts.len() {
            i = self.instrument_stmt(
                i,
                body,
                exit_counter,
                ppt_name,
                dtrace_param_blocks,
                &param_to_block_idx,
                &ret_ty,
                daikon_tmp_counter,
            );
        }
    }

    // Walk the function body and insert dtrace calls at
    // the beginning and at exit points.
    // * `ppt_name` - Program point name.
    // * `body` - The block to instrument.
    // * `dtrace_param_blocks` - Vec of Strings containing dtrace
    //                           calls for each parameter.
    // * `param_to_block_idx` - No description.
    // * `ret_ty` - Function return type.
    // * `daikon_tmp_counter` - Stores the next label for a temporary
    //                          variable used in the instrumented code.
    fn instrument_fn_body(
        &mut self,
        ppt_name: &str,
        body: &mut Box<Block>,
        dtrace_param_blocks: &mut Vec<String>,
        param_to_block_idx: FxHashMap<String, i32>,
        ret_ty: &FnRetTy,
        daikon_tmp_counter: &mut u32,
    ) {
        let mut i = 0;

        // FIXME: implement a similar fix for this
        // How nonces should be done--
        //   lock a global counter shared by all threads
        //   store its current value
        //   increment it
        //   unlock
        //   use the stored value at all exit points in this function
        // Currently there is a nonce counter per file which is not correct.
        i = self.insert_into_block(i, &String::from(DTRACE_INIT_NONCE), body);

        // ${program_point} -> ppt_name
        let entry =
            substitute(FxHashMap::from_iter([("${program_point}", ppt_name)]), DTRACE_ENTRY);

        i = self.insert_into_block(i, &entry, body);
        for param_block in &mut *dtrace_param_blocks {
            i = self.insert_into_block(i, &param_block, body);
        }

        i = self.insert_into_block(i, &String::from(DTRACE_NEWLINE), body);

        // Before instrumenting fn body, turn implicit void return into "return;".
        // This may be unreachable in some situations like
        // fn foo(t: bool) { if t { return; } else { return; } }.
        // In this situation we should not add a return stmt, but
        // we do not detect this yet. Maybe there is a better solution
        // to detecting the end of a function that returns void.
        match &ret_ty {
            FnRetTy::Default(_) => {
                if body.stmts.is_empty() || !last_stmt_is_void_return(body) {
                    self.append_to_block(&String::from(DTRACE_VOID_RETURN), body);
                }
            }
            _ => {}
        }

        let mut exit_counter = 1;

        // Assuming no unreachable statements.
        while i < body.stmts.len() {
            i = self.instrument_stmt(
                i,
                body,
                &mut exit_counter,
                ppt_name,
                dtrace_param_blocks,
                &param_to_block_idx,
                &ret_ty,
                daikon_tmp_counter,
            )
        }
    }

    // Convert &str to items. We create a new file dtrace_parserX each time we want to
    // parse some new items vec. Diagnostics sometimes point to these files for unknown
    // reasons.
    pub fn parse_items_from_source_str(&self, source: &str) -> PResult<'a, ThinVec<Box<Item>>> {
        self.parse_items_from_source_string(String::from(source))
    }

    // Convert String to items. We create a new file dtrace_parserX each time we want to
    // parse some new items vec. Diagnostics sometimes point to these files for unknown
    // reasons.
    // * `items` - String with items to parse.
    pub fn parse_items_from_source_string(&self, items: String) -> PResult<'a, ThinVec<Box<Item>>> {
        let count = *PARSER_COUNTER.lock().unwrap();
        //let psess = ParseSess::new();
        let mut tmp_parser = unwrap_or_emit_fatal(new_parser_from_source_str(
            &self.psess,
            rustc_span::FileName::Custom(format!("{}{}", "dtrace_parser", count.to_string())),
            items,
            StripTokens::Nothing,
        ));

        *PARSER_COUNTER.lock().unwrap() += 1;

        let mut tmp_items: ThinVec<Box<_>> = ThinVec::new();

        // Parse from str.
        loop {
            while tmp_parser.maybe_consume_incorrect_semicolon(tmp_items.last().map(|x| &**x)) {}
            let Some(item) = tmp_parser.parse_item(ForceCollect::No, AllowConstBlockItems::Yes)?
            else {
                break;
            };
            tmp_items.push(item);
        }

        Ok(tmp_items)
    }
}

// The main visitor routines and entry-points for function and struct
// instrumentation.
impl<'a> MutVisitor for DaikonDtraceVisitor<'a> {
    // Process the function signature to generate calls to log arguments and
    // return value.
    // Visit the function body and insert calls at exit points via
    // DaikonDtraceVisitor::insert_into_block.
    // * `fk` - The function kind, i.e., function or closure.
    fn visit_fn(
        &mut self,
        mut fk: FnKind<'_>,
        _attrs: &rustc_ast::AttrVec,
        _span: rustc_span::Span,
        _id: rustc_ast::NodeId,
    ) {
        // Walk the function body looking for return statements and adding
        // instrumentation.
        match &mut fk {
            FnKind::Fn(_, _, f) => {
                if !f.generics.params.is_empty() {
                    // Skip generics for now.
                    return;
                }
                let ppt_name = f.ident.as_str();
                if ppt_name == "execute" {
                    return;
                }
                let mut daikon_tmp_counter = 0;
                // get block of dtrace chunks -- one for each param (in a String, not good).
                let mut dtrace_param_blocks =
                    self.fn_sig_to_dtrace_code(&f.sig.decl, &mut daikon_tmp_counter);
                let param_to_block_idx = map_params(&f.sig.decl);
                match &mut f.body {
                    None => {}
                    Some(body) => {
                        self.instrument_fn_body(
                            ppt_name,
                            body,
                            &mut dtrace_param_blocks,
                            param_to_block_idx,
                            &f.sig.decl.output,
                            &mut daikon_tmp_counter,
                        );
                    }
                };
            }
            // FIXME: instrument closures.
            FnKind::Closure(_, _, _, _) => {}
        };

        // we didn't look for struct definitions while walking, so now we prepare a new ScopeType::FnBody
        // and walk this function to generate impl blocks
        self.enter_fn_body();

        // We need to preserve fk and avoid moving it, so we cannot call walk_fn
        // on fk.
        // To do this, we can fully clone fk and walk that instead so we can
        // modify fk after calling walk_fn.
        // FIXME: this seems bad for performance, cloning a whole function AST node.
        let fk_clone = match &mut fk {
            FnKind::Fn(ctxt, vis, f) => FnKind::Fn(ctxt.clone(), &mut vis.clone(), &mut f.clone()),
            FnKind::Closure(binder, coro_kind, decl, expr) => match &coro_kind {
                Some(coro_kind) => FnKind::Closure(
                    &mut binder.clone(),
                    &mut Some(coro_kind.clone()),
                    &mut decl.clone(),
                    &mut expr.clone(),
                ),
                None => FnKind::Closure(
                    &mut binder.clone(),
                    &mut None,
                    &mut decl.clone(),
                    &mut expr.clone(),
                ),
            },
        };
        mut_visit::walk_fn(self, fk_clone);

        // now we can mutate fk like before.
        match &mut fk {
            FnKind::Fn(_, _, f) => match &mut f.body {
                Some(body) => {
                    self.pop_back_into_stmts_front(&mut body.stmts);
                }
                None => {
                    self.scope_stack.pop_back();
                }
            },
            _ => {
                self.scope_stack.pop_back();
            }
        };
    }

    // Visit all structs and generate new impl blocks with dtrace
    // routine definitions.
    // FIXME: look up struct names in a /tmp file to determine
    //       whether to continue or not.
    // * `item` - An item to maybe instrument.
    fn visit_item(&mut self, item: &mut Item) {
        let mut inline_mod_p = false;

        match &mut item.kind {
            ItemKind::Enum(_ident, _generics, _enum_def) => {}
            ItemKind::Struct(ident, generics, variant_data) => match variant_data {
                VariantData::Struct { fields, recovered: _recovered } => {
                    let mut the_path = Path::from_ident(ident.clone());
                    let mut the_args: ThinVec<AngleBracketedArg> = ThinVec::new();
                    for i in 0..generics.params.len() {
                        match &generics.params[i].kind {
                            GenericParamKind::Lifetime => {
                                the_args.push(AngleBracketedArg::Arg(GenericArg::Lifetime(
                                    Lifetime {
                                        id: NodeId::MAX_AS_U32.into(),
                                        ident: generics.params[i].ident.clone(),
                                    },
                                )));
                            }
                            GenericParamKind::Type { default: _ } => {
                                // Fix generic types for now.
                                // TODO: return.
                                // return;
                                panic!("Struct has type generic arg.")
                            }
                            GenericParamKind::Const { ty: _, span: _, default: _ } => {
                                panic!("Enum has const generic arg.")
                            }
                        }
                    }
                    let angle_bracketed_args =
                        AngleBracketedArgs { span: item.span.clone(), args: the_args };
                    the_path.segments[0].args =
                        Some(Box::new(GenericArgs::AngleBracketed(angle_bracketed_args)));
                    let the_ty = Ty {
                        id: NodeId::MAX_AS_U32.into(),
                        kind: TyKind::Path(None, the_path.clone()),
                        span: item.span.clone(),
                        tokens: None,
                    };

                    let impl_item = self.gen_impl(fields, &the_ty, &generics);

                    // Now we push this to the back of scope_stack.
                    // This is the only time we ever see a struct definition. i.e., this
                    // is the only place where gen_impl is ever called. So we need
                    // not push generated impls anywhere else. We only push new/empty scopes
                    // everywhere else. Note, we don't need to pop these impls until we reach
                    // the end of our current scope, e.g., end of function scope. Things are
                    // popped correctly in, e.g., visit_fn.
                    if self.scope_type_requires_items_p() {
                        self.push_impl_item(impl_item);
                    } else {
                        let impl_stmt = Stmt {
                            id: NodeId::MAX_AS_U32.into(),
                            kind: StmtKind::Item(impl_item),
                            span: item.span.clone(),
                        };
                        self.push_impl_stmt(impl_stmt);
                    }

                    // also, structs cannot be defined in structs, so we don't
                    // need to enter a new scope here.
                    // NOTE: in decls generation, you absolutely need to enter
                    // a scope here for deducing type of seen self parameters.
                }
                VariantData::Tuple(_, _) => {}
                _ => {}
            },
            ItemKind::Union(_ident, _generics, _variant_data) => {}
            ItemKind::Mod(_, _, mod_kind) => match &mod_kind {
                ModKind::Loaded(_, inline, _) => match &inline {
                    Inline::Yes => {
                        inline_mod_p = true;
                    }
                    _ => {}
                },
                _ => {}
            },
            _ => {}
        };

        if !inline_mod_p {
            mut_visit::walk_item(self, item);
        }

        // If there are any items where structs can be defined, match &mut item.kind again
        // here and dump the new impls into those items.
        //match &mut item.kind {
    }
}

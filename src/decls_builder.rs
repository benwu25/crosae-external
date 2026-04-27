use std::collections::VecDeque;
use std::fmt::Write;
use std::io::Write as FileWrite;
use std::mem;
use std::sync::{LazyLock, Mutex};

use crate::OUTPUT_PREFIX;
use crate::daikon_strs::*;
use rustc_ast::visit;
use rustc_ast::visit::*;
use rustc_ast::*;
use rustc_data_structures::fx::FxHashMap;
use rustc_data_structures::thin_vec::{ThinVec, thin_vec};

// Given a parameter pat, return its identifier name in a String.
// * `pat` - Pat struct representing a parameter identifier.
fn get_param_ident(pat: &Box<Pat>) -> String {
    match &pat.kind {
        PatKind::Ident(_mode, ident, None) => String::from(ident.as_str()),
        _ => panic!("Formal arg does not have simple identifier"),
    }
}

// Given a Rust type, return its Daikon rep-type.
// For non-primitive types, return an empty string.
// E.g.,
// i8 -> int
// i32 -> int
// char -> char
// bool -> boolean
// &str -> java.lang.String
// String -> java.lang.String
// * `ty_str` - String representing the type of a parameter
//              or return variable.
fn as_prim_rep_type(ty_str: &str) -> &str {
    if ty_str == I8
        || ty_str == I16
        || ty_str == I32
        || ty_str == I64
        || ty_str == I128
        || ty_str == ISIZE
        || ty_str == U8
        || ty_str == U16
        || ty_str == U32
        || ty_str == U64
        || ty_str == U128
        || ty_str == USIZE
    {
        return "int";
    } else if ty_str == F32 || ty_str == F64 {
        return "";
    } else if ty_str == CHAR {
        return "char";
    } else if ty_str == BOOL {
        return "boolean";
    } else if ty_str == UNIT {
        return "";
    } else if ty_str == STR || ty_str == STRING {
        return "java.lang.String";
    }
    ""
}

// Given the template arguments to a Vec or array, return a RepType
// enum representing the Vec/array.
// * `generic_args` - Generic args to a Vec parameter.
fn vec_generics_to_rust_type(
    generic_args: &Path,
    visitor: Option<&DaikonDeclsVisitor<'_>>,
) -> RepType {
    let mut is_ref = false;
    match &generic_args.segments[generic_args.segments.len() - 1].args {
        None => panic!("Vec args has no type name"),
        Some(args) => match &**args {
            GenericArgs::AngleBracketed(brack_args) => match &brack_args.args[0] {
                AngleBracketedArg::Arg(arg) => match &arg {
                    GenericArg::Type(arg_type) => {
                        match &get_rep_type(&arg_type.kind, &mut is_ref, visitor) {
                            RepType::Prim(arg_p_type) => RepType::PrimArray(arg_p_type.to_string()),
                            RepType::HashCodeStruct(struct_type) => {
                                RepType::HashCodeArray(struct_type.to_string())
                            }
                            _ => panic!("Multi-dim vec/array not supported"),
                        }
                    }
                    _ => panic!("Grok args failed 1"),
                },
                _ => panic!("Grok args failed 2"),
            },
            _ => panic!("Grok args failed 3"),
        },
    }
}

// Capable of representing the rep-type of a Rust type.
// String payload represents the corresponding "Java" type
// i32 -> Prim("int")
// &[i32] -> PrimArray("int")
// [X; 2] -> HashCodeArray("X")
// &'a X -> HashCodeStruct("X")
#[derive(PartialEq)]
enum RepType {
    Prim(String),
    PrimArray(String),
    HashCodeArray(String),
    HashCodeStruct(String),
    Skip,
}

// Given a Rust type kind, return its RepType. Also note whether the type
// is a reference by setting is_ref.
// * `kind` - Represents the actual type of a parameter in the Rust language.
// * `is_ref` - Used to determine reference qualifiers on the type.
fn get_rep_type(
    kind: &TyKind,
    is_ref: &mut bool,
    visitor: Option<&DaikonDeclsVisitor<'_>>,
) -> RepType {
    match &kind {
        TyKind::Array(arr_type, _) => match &get_rep_type(&arr_type.kind, is_ref, visitor) {
            RepType::Prim(p_type) => RepType::PrimArray(String::from(p_type)),
            RepType::HashCodeStruct(basic_type) => RepType::HashCodeArray(String::from(basic_type)),
            _ => panic!("higher-dim arrays not supported"),
        },
        TyKind::Slice(arr_type) => match &get_rep_type(&arr_type.kind, is_ref, visitor) {
            RepType::Prim(p_type) => RepType::PrimArray(String::from(p_type)),
            RepType::HashCodeStruct(basic_type) => RepType::HashCodeArray(String::from(basic_type)),
            _ => panic!("higher-dim arrays not supported"),
        },
        TyKind::Ptr(_) => todo!(),
        TyKind::Ref(_, mut_ty) => {
            *is_ref = true;
            return get_rep_type(&mut_ty.ty.kind, is_ref, visitor);
        }
        TyKind::Path(_, path) => {
            if path.segments.is_empty() {
                panic!("Path has no type");
            }
            let ty_string = path.segments[path.segments.len() - 1].ident.as_str();
            let maybe_prim_rep = as_prim_rep_type(ty_string);
            if maybe_prim_rep != "" {
                return RepType::Prim(String::from(maybe_prim_rep));
            }
            if ty_string == VEC {
                return vec_generics_to_rust_type(&path, visitor);
            }
            return RepType::HashCodeStruct(String::from(ty_string));
        }
        TyKind::ImplicitSelf => {
            // Query for the impl we are currently in by accessing scope_stack.
            match &visitor {
                None => panic!("Cannot access scope_stack in get_rep_type"),
                Some(visitor) => match &visitor.scope_stack.back() {
                    Some(plain_struct) => RepType::HashCodeStruct(String::from(*plain_struct)),
                    None => panic!("scope_stack has no name for this struct"),
                },
            }
        }
        TyKind::ImplTrait(_, _) => {
            // A bunch of types we want to skip for Daikon.
            RepType::Skip
        }
        _ => todo!(),
    }
}

// Unused.
fn map_params(decl: &Box<FnDecl>) -> FxHashMap<String, i32> {
    let mut res = FxHashMap::default();
    for i in 0..decl.inputs.len() {
        res.insert(get_param_ident(&decl.inputs[i].pat), i as i32);
    }
    res
}

// Immutable visitor to visit all structs and build a map data structure.
// FIXME: remove, we will use a /tmp file instead.
pub struct DeclsHashMapBuilder<'a> {
    pub map: &'a mut FxHashMap<String, Box<Item>>,
}

impl<'a> Visitor<'a> for DeclsHashMapBuilder<'a> {
    // Visit structs and fill hash map.
    fn visit_item(&mut self, item: &'a Item) {
        match &item.kind {
            ItemKind::Struct(ident, _, variant_data) => match variant_data {
                VariantData::Struct {
                    fields: _,
                    recovered: _,
                } => {
                    self.map
                        .insert(String::from(ident.as_str()), Box::new(item.clone()));
                }
                VariantData::Tuple(_, _) => {}
                _ => {}
            },
            _ => {}
        }

        visit::walk_item(self, item);
    }
}

// Main struct for walking functions to write the decls file.
// map allows for quick retrieval of struct fields when a struct
// parameter is encountered.
// depth_limit tells us when to stop writing decls for recursive structs.
pub struct DaikonDeclsVisitor<'a> {
    pub map: &'a FxHashMap<String, Box<Item>>,
    pub depth_limit: u32,
    pub scope_stack: &'a mut VecDeque<String>,
}

// Represents a parameter or return value which must be written to decls.
// map: map from String to struct definition with field declarations.
// var_name: parameter name, or "return" for return values.
// dec_type: Declared type of the value (dec-type for Daikon).
// rep_type: Rep type of the value (rep-type for Daikon).
// struct_name: If the value is a struct, contains the struct type name for lookup,
//      otherwise None.
// field_decls: If the value is a struct, represents decl records for the
//              fields of this struct.
// contents: If the value is Vec or array, a decls record for the contents
//           of this outer container.
// Note: it is maintained that only one of field_decls or contents will be non-empty.
struct TopLevlDecl<'a> {
    pub map: &'a FxHashMap<String, Box<Item>>,
    pub var_name: String,
    pub dec_type: String,
    pub rep_type: String,
    pub struct_name: Option<String>, // struct name for looking up structs if this is a struct.
    pub field_decls: Vec<FieldDecl<'a>>,
    pub contents: Option<Box<ArrayContents<'a>>>,
}

// Represents a field decl of a struct at some arb. depth.
// enclosing_var: name of the struct variable which contains this field.
// field_name: name of this field.
struct FieldDecl<'a> {
    pub decl: Box<TopLevlDecl<'a>>,
    pub enclosing_var: String,
    pub field_name: String,
}

// Represents the array contents decl record (i.e., arr[..] or arr[..].g rather than arr).
// enclosing_var: name of the outer container for this array or Vec.
// sub_contents: If the top-level variable is an array of structs, we need ArrayContents for each field.
// See TopLevlDecl for other fields.
struct ArrayContents<'a> {
    /*    pub map: &'a FxHashMap<String, Box<Item>>, pub var_name: String,
    pub dec_type: String,
    pub rep_type: String,
    pub struct_name: Option<String>, */
    pub decl: Box<TopLevlDecl<'a>>,
    pub enclosing_var: String,
    pub sub_contents: Option<Vec<ArrayContents<'a>>>, // only if this is a hashcode[], for printing subfield array records.
}

impl<'a> ArrayContents<'a> {
    // Append an ArrayContents to the decls file.
    fn write(&mut self) {
        match &mut *DECLS.lock().unwrap() {
            None => panic!("Cannot open decls"),
            Some(decls) => {
                if self.decl.var_name == "false" {
                    return;
                }

                writeln!(decls, "variable {}", self.decl.var_name).ok();
                writeln!(decls, "  var-kind array").ok();
                writeln!(decls, "  enclosing-var {}", self.enclosing_var).ok();
                writeln!(decls, "  array 1").ok();
                writeln!(decls, "  dec-type {}", self.decl.dec_type).ok();
                writeln!(decls, "  rep-type {}", self.decl.rep_type).ok();
                writeln!(decls, "  comparability -1").ok();
            }
        }

        match &mut self.sub_contents {
            None => {}
            Some(sub_contents) => {
                for i in 0..sub_contents.len() {
                    sub_contents[i].write();
                }
            }
        }
    }

    // If the top-level variable is an array of structs, use our struct_name to fetch field definitions
    // of our struct type.
    // * `write_p` - Output parameter to indicate if self.write() should output
    //               anything to the decls file when called.
    fn get_fields(&self, write_p: &mut bool) -> ThinVec<FieldDef> {
        // use self.struct_name to look up who we are.
        match &self.decl.struct_name {
            None => panic!("No struct_name for get_fields"),
            Some(struct_name) => {
                let struct_item = self.decl.map.get(struct_name);
                match &struct_item {
                    None => {
                        // This is an Enum or Union or ?
                        *write_p = false;
                        ThinVec::new()
                    }
                    Some(struct_item) => match &struct_item.kind {
                        ItemKind::Struct(_, _, variant_data) => match variant_data {
                            VariantData::Struct {
                                fields,
                                recovered: _,
                            } => fields.clone(),
                            _ => panic!("Struct is not VariantData::Struct"),
                        },
                        _ => panic!("struct_item is not a struct"),
                    },
                }
            }
        }
    }

    // If var is an array of structs, recursively populate sub_contents by creating
    // a new ArrayContents for each field.
    // * `depth_limit` - Argument to limit writing recursively defined structs
    //                   to the decls file.
    // * `write_p` - Output parameter to indicate if self.write() should output
    //               anything to the decls file when called.
    fn build_contents(&mut self, depth_limit: u32, write_p: &mut bool) {
        if depth_limit == 0 {
            return;
        }

        // fields of the struct in this array.
        let fields = self.get_fields(write_p);
        if !*write_p {
            return;
        }

        for i in 0..fields.len() {
            let field_name = match &fields[i].ident {
                Some(field_ident) => String::from(field_ident.as_str()),
                None => panic!("Field has no identifier"),
            };
            let var_name = format!("{}.{}", self.decl.var_name, field_name);
            let mut is_ref = false;
            let mut write_p = true;
            let var_decl = match &get_rep_type(&fields[i].ty.kind, &mut is_ref, None) {
                RepType::Prim(p_type) => ArrayContents {
                    decl: Box::new(TopLevlDecl {
                        map: self.decl.map,
                        var_name: var_name.clone(),
                        dec_type: format!("{}[]", p_type),
                        rep_type: format!("{}[]", p_type),
                        struct_name: None,
                        field_decls: Vec::new(),
                        contents: None,
                    }),
                    enclosing_var: self.decl.var_name.clone(),
                    sub_contents: None,
                },
                RepType::HashCodeStruct(ty_string) => {
                    let mut tmp = ArrayContents {
                        decl: Box::new(TopLevlDecl {
                            map: self.decl.map,
                            var_name: var_name.clone(),
                            dec_type: format!("{}[]", ty_string),
                            rep_type: String::from("hashcode[]"),
                            struct_name: Some(ty_string.clone()),
                            field_decls: Vec::new(),
                            contents: None,
                        }),
                        enclosing_var: self.decl.var_name.clone(),
                        sub_contents: Some(Vec::new()),
                    };
                    tmp.build_contents(depth_limit - 1, &mut write_p);

                    // Error checking.
                    if !write_p {
                        // Any "fields" are invalid, but tmp could be an enum/union and pointer is valid.
                        match &mut tmp.sub_contents {
                            None => panic!("Expected some field_decls 1"), // expected sub_contents?
                            Some(sub_contents) => {
                                for j in 0..sub_contents.len() {
                                    sub_contents[j].decl.var_name = String::from("false");
                                }
                            }
                        }
                    }
                    if ty_string.starts_with("Option") || ty_string.starts_with("Result") {
                        // this record is also invalid.
                        tmp.decl.var_name = String::from("false");
                    }
                    tmp
                }
                RepType::PrimArray(_) => {
                    // only print pointers.
                    ArrayContents {
                        decl: Box::new(TopLevlDecl {
                            map: self.decl.map,
                            var_name: var_name.clone(),
                            dec_type: String::from("<higher-dim-array>"),
                            rep_type: String::from("hashcode[]"),
                            struct_name: None, // we shouldn't be using this in write.
                            field_decls: Vec::new(),
                            contents: None,
                        }),
                        enclosing_var: self.decl.var_name.clone(),
                        sub_contents: None,
                    }
                }
                RepType::HashCodeArray(_) => {
                    // only print pointers.
                    ArrayContents {
                        decl: Box::new(TopLevlDecl {
                            map: self.decl.map,
                            var_name: var_name.clone(),
                            dec_type: String::from("<higher-dim-array>"),
                            rep_type: String::from("hashcode[]"),
                            struct_name: None,
                            field_decls: Vec::new(),
                            contents: None,
                        }),
                        enclosing_var: self.decl.var_name.clone(),
                        sub_contents: None,
                    }
                }
                RepType::Skip => {
                    continue;
                }
            };
            match &mut self.sub_contents {
                None => panic!("No sub_contents in build_contents"),
                Some(sub_contents) => {
                    sub_contents.push(var_decl);
                }
            }
        }
    }
}

impl<'a> FieldDecl<'a> {
    // Write this entire FieldDecl to the decls file.
    fn write(&mut self) {
        match &mut *DECLS.lock().unwrap() {
            None => panic!("Cannot open decls"),
            Some(decls) => {
                if self.decl.var_name == "false" {
                    return;
                }

                writeln!(decls, "variable {}", self.decl.var_name).ok();
                writeln!(decls, "  var-kind field {}", self.field_name).ok();
                writeln!(decls, "  enclosing-var {}", self.enclosing_var).ok();
                writeln!(decls, "  dec-type {}", self.decl.dec_type).ok();
                writeln!(decls, "  rep-type {}", self.decl.rep_type).ok();
                writeln!(decls, "  comparability -1").ok();
            }
        }

        match &mut self.decl.contents {
            None => {
                for i in 0..self.decl.field_decls.len() {
                    self.decl.field_decls[i].write();
                }
                return;
            }
            Some(contents) => {
                contents.write();
            }
        }
    }

    // If this FieldDecl represents a struct field, recursively build up our FieldDecl tree
    // by creating a new FieldDecl for each field and recursing for nested struct types.
    // * `depth_limit` - Argument to limit writing recursively defined structs
    //                   to the decls file.
    // * `write_p` - Output parameter to indicate if self.write() should output
    //               anything to the decls file when called.
    fn construct_field_decls(&mut self, depth_limit: u32, write_p: &mut bool) {
        // enclosing_var and field_name have been set, so just take care of decl.
        self.decl.construct_field_decls(depth_limit, write_p);
    }
}

impl<'a> TopLevlDecl<'a> {
    // Write this entire TopLevlDecl to the decls file.
    fn write(&mut self) {
        match &mut *DECLS.lock().unwrap() {
            None => panic!("Cannot open decls"),
            Some(decls) => {
                if self.var_name == "false" {
                    return;
                }

                writeln!(decls, "variable {}", self.var_name).ok();
                writeln!(decls, "  var-kind variable").ok();
                writeln!(decls, "  dec-type {}", self.dec_type).ok();
                writeln!(decls, "  rep-type {}", self.rep_type).ok();
                writeln!(decls, "  flags is_param").ok();
                writeln!(decls, "  comparability -1").ok();
            }
        }

        match &mut self.contents {
            None => {
                for i in 0..self.field_decls.len() {
                    self.field_decls[i].write();
                }
                return;
            }
            Some(contents) => {
                contents.write();
            }
        }
    }

    // If field_decls is Some and sub_contents is None (top-level is a struct variable),
    // recursively build declarations for the fields.
    // * `depth_limit` - Argument to limit writing recursively defined structs
    //                   to the decls file.
    // * `write_p` - Output parameter to indicate if self.write() should output
    //               anything to the decls file when called.
    fn construct_field_decls(&mut self, depth_limit: u32, write_p: &mut bool) {
        if depth_limit == 0 {
            // Stop recursively constructing FieldDecl tree and return
            // for writing.
            return;
        }

        let fields = self.get_fields(write_p);
        if !*write_p {
            return;
        }

        for i in 0..fields.len() {
            let field_name = match &fields[i].ident {
                Some(field_ident) => String::from(field_ident.as_str()),
                None => panic!("Field has no identifier"),
            };
            let var_name = format!("{}.{}", self.var_name, field_name);
            let mut is_ref = false;
            let mut write_p = true;
            let var_decl = match &get_rep_type(&fields[i].ty.kind, &mut is_ref, None) {
                RepType::Prim(p_type) => {
                    let tmp_toplevl = TopLevlDecl {
                        map: self.map,
                        var_name: var_name.clone(),
                        dec_type: p_type.clone(),
                        rep_type: p_type.clone(),
                        struct_name: None,
                        field_decls: Vec::new(),
                        contents: None,
                    };
                    FieldDecl {
                        decl: Box::new(tmp_toplevl),
                        enclosing_var: self.var_name.clone(),
                        field_name: field_name.clone(),
                    } // Ready to write.
                }
                RepType::HashCodeStruct(ty_string) => {
                    let tmp_toplevl = TopLevlDecl {
                        map: self.map,
                        var_name: var_name.clone(),
                        dec_type: ty_string.clone(),
                        rep_type: String::from("hashcode"),
                        struct_name: Some(ty_string.clone()),
                        field_decls: Vec::new(),
                        contents: None,
                    };
                    let mut tmp = FieldDecl {
                        decl: Box::new(tmp_toplevl),
                        enclosing_var: self.var_name.clone(),
                        field_name: field_name.clone(),
                    };
                    tmp.construct_field_decls(depth_limit - 1, &mut write_p);

                    // Error checking.
                    if !write_p {
                        // Any "fields" are invalid, but tmp could be an enum/union and pointer is valid.
                        match &tmp.decl.contents {
                            None => {
                                for j in 0..tmp.decl.field_decls.len() {
                                    tmp.decl.field_decls[j].decl.var_name = String::from("false");
                                }
                            }
                            Some(_) => panic!("Expected some field_decls 2"),
                        }
                    }
                    if ty_string.starts_with("Option") || ty_string.starts_with("Result") {
                        // this record is also invalid.
                        tmp.decl.var_name = String::from("false");
                    }
                    tmp
                }
                RepType::PrimArray(p_type) => {
                    let tmp_toplevl = TopLevlDecl {
                        map: self.map,
                        var_name: var_name.clone(),
                        dec_type: format!("{}[]", p_type),
                        rep_type: String::from("hashcode"),
                        struct_name: None,
                        field_decls: Vec::new(),
                        contents: Some(Box::new(ArrayContents {
                            decl: Box::new(TopLevlDecl {
                                map: self.map,
                                var_name: format!("{}[..]", var_name),
                                dec_type: format!("{}[]", p_type),
                                rep_type: format!("{}[]", p_type),
                                struct_name: None,
                                field_decls: Vec::new(),
                                contents: None,
                            }),
                            enclosing_var: var_name.clone(),
                            sub_contents: None,
                        })), // Ready to write.
                    };
                    FieldDecl {
                        decl: Box::new(tmp_toplevl),
                        enclosing_var: self.var_name.clone(),
                        field_name: field_name.clone(),
                    }
                }
                RepType::HashCodeArray(ty_string) => {
                    let tmp_toplevl = TopLevlDecl {
                        map: self.map,
                        var_name: var_name.clone(),
                        dec_type: format!("{}[]", ty_string),
                        rep_type: String::from("hashcode"),
                        struct_name: Some(ty_string.clone()),
                        field_decls: Vec::new(),
                        contents: Some(Box::new(ArrayContents {
                            decl: Box::new(TopLevlDecl {
                                map: self.map,
                                var_name: format!("{}[..]", var_name),
                                dec_type: format!("{}[]", ty_string),
                                rep_type: String::from("hashcode[]"),
                                struct_name: Some(ty_string.clone()),
                                field_decls: Vec::new(),
                                contents: None,
                            }),
                            enclosing_var: var_name.clone(),
                            sub_contents: Some(Vec::new()),
                        })),
                    };
                    let mut tmp = FieldDecl {
                        decl: Box::new(tmp_toplevl),
                        enclosing_var: self.var_name.clone(),
                        field_name: field_name.clone(),
                    };
                    match &mut tmp.decl.contents {
                        None => panic!("Missing contents field for HashCodeArray"),
                        Some(contents) => {
                            contents.build_contents(depth_limit - 1, &mut write_p);

                            // Error checking.
                            if !write_p {
                                // Any "fields" are invalid, but tmp could be an enum/union and pointers is valid.
                                match &mut contents.sub_contents {
                                    None => panic!("Expected some field_decls 1"), // field_decls?
                                    Some(sub_contents) => {
                                        for j in 0..sub_contents.len() {
                                            sub_contents[j].decl.var_name = String::from("false");
                                        }
                                    }
                                }
                            }

                            if ty_string.starts_with("Option") || ty_string.starts_with("Result") {
                                // this record is also invalid.
                                tmp.decl.var_name = String::from("false");
                            }
                        }
                    }
                    tmp
                }
                RepType::Skip => {
                    continue;
                }
            };
            match &self.contents {
                None => {
                    self.field_decls.push(var_decl);
                }
                Some(_) => panic!("No field_decls in construct_field_decls"),
            }
        }
    }

    // If the top-level var is a struct variable, use our struct_name to get field definitions
    // for our struct type.
    // * `write_p` - Output parameter to indicate if self.write() should output
    //               anything to the decls file when called.
    fn get_fields(&self, write_p: &mut bool) -> ThinVec<FieldDef> {
        // use self.struct_name to look up who we are.
        match &self.struct_name {
            None => panic!("No struct_name for get_fields"),
            Some(struct_name) => {
                let struct_item = self.map.get(struct_name);
                match &struct_item {
                    None => {
                        *write_p = false;
                        ThinVec::new()
                    }
                    Some(struct_item) => match &struct_item.kind {
                        ItemKind::Struct(_, _, variant_data) => match variant_data {
                            VariantData::Struct {
                                fields,
                                recovered: _,
                            } => fields.clone(),
                            _ => panic!("Struct is not VariantData::Struct"),
                        },
                        _ => panic!("struct_item is not a struct"),
                    },
                }
            }
        }
    }
}

// Helper to write function entries into the decls file.
// * `ppt_name` - Program point name.
fn write_entry(ppt_name: &str) {
    match &mut *DECLS.lock().unwrap() {
        None => panic!("Cannot access decls"),
        Some(decls) => {
            writeln!(decls, "ppt {}:::ENTER", ppt_name).ok();
            writeln!(decls, "ppt-type enter").ok();
        }
    }
}

// Helper to write function exits into the decls file.
// * `ppt_name` - Program point name.
// * `exit_counter` - Unique numeric identifier for an exit ppt.
fn write_exit(ppt_name: &str, exit_counter: usize) {
    match &mut *DECLS.lock().unwrap() {
        None => panic!("Cannot access decls"),
        Some(decls) => {
            writeln!(decls, "ppt {}:::EXIT{}", ppt_name, exit_counter).ok();
            writeln!(decls, "ppt-type exit").ok();
        }
    }
}

// Helper to add a newline in the decls file.
pub fn write_newline() {
    match &mut *DECLS.lock().unwrap() {
        None => panic!("Cannot access decls"),
        Some(decls) => {
            writeln!(decls, "").ok();
        }
    }
}

// Helper to write metadata header into the decls file.
pub fn write_header() {
    match &mut *DECLS.lock().unwrap() {
        None => panic!("Cannot access decls"),
        Some(decls) => {
            writeln!(decls, "decl-version 2.0").ok();
            writeln!(decls, "input-language Rust").ok();
            writeln!(decls, "var-comparability implicit").ok();
        }
    }
}

impl<'a> DaikonDeclsVisitor<'a> {
    // * `plain_struct` - The struct identifier whose scope
    //                    we are about to enter.
    fn push_struct(&mut self, plain_struct: String) {
        self.scope_stack.push_back(plain_struct);
    }

    // Pop the top of the scope_stack.
    fn pop_struct(&mut self) {
        self.scope_stack.pop_back();
    }

    // Walk an if expression looking for returns and
    // write exit-ppt declarations when found.
    // See rustc_parse::parser::item::instrument_if_stmt.
    // * `expr` - If expression.
    // * `exit_counter` - Gives the previously seen number of exit ppts.
    // * `ppt_name` - The ppt name.
    // * `param_decls` - Vec of TopLevlDecl representing data
    //                    needed to write variable declarations to the
    //                    decls file at program points.
    // * `param_to_block_idx` - Map of param identifiers to idx into
    //                          dtrace_param_blocks.
    // * `ret_ty` - Return type of the function.
    fn if_stmt_to_decls(
        &mut self,
        expr: &Box<Expr>,
        exit_counter: &mut usize,
        ppt_name: &str,
        param_decls: &mut Vec<Box<TopLevlDecl<'_>>>,
        param_to_block_idx: &FxHashMap<String, i32>,
        ret_ty: &FnRetTy,
    ) {
        match &expr.kind {
            ExprKind::Block(block, _) => {
                self.block_to_decls(
                    ppt_name,
                    block,
                    param_decls,
                    &param_to_block_idx,
                    &ret_ty,
                    exit_counter,
                );
            }
            ExprKind::If(_, then_block, elif_block) => {
                self.block_to_decls(
                    ppt_name,
                    then_block,
                    param_decls,
                    &param_to_block_idx,
                    &ret_ty,
                    exit_counter,
                );
                match &elif_block {
                    Some(elif_block) => {
                        self.if_stmt_to_decls(
                            elif_block,
                            exit_counter,
                            ppt_name,
                            param_decls,
                            &param_to_block_idx,
                            &ret_ty,
                        );
                    }
                    None => {}
                }
            }
            // See
            // https://doc.rust-lang.org/nightly/nightly-rustc/rustc_ast/ast/enum.ExprKind.html.
            // If we really are processing in if-else tree, no other ExprKind should show up.
            _ => panic!("Internal error handling if stmt!"),
        }
    }

    // Process an entire stmt to find an exit point to write an exit-ppt declaration
    // or recurse on block stmts.
    // See rustc_parse::parser::item::instrument_stmt.
    // * `i` - Index into block of the stmt to check.
    // * `exit_counter` - Gives the previously seen number of exit ppts.
    // * `ppt_name` - The ppt name.
    // * `param_decls` - Vec of TopLevlDecl representing data
    //                    needed to write variable declarations to the
    //                    decls file at program points.
    // * `param_to_block_idx` - Map of param identifiers to idx into
    //                          dtrace_param_blocks.
    // * `ret_ty` - Return type of the function.
    fn stmt_to_decls(
        &mut self,
        i: usize,
        body: &Box<Block>,
        exit_counter: &mut usize,
        ppt_name: &str,
        param_decls: &mut Vec<Box<TopLevlDecl<'_>>>,
        param_to_block_idx: &FxHashMap<String, i32>,
        ret_ty: &FnRetTy,
    ) -> usize {
        let mut block_idx = i;
        match &body.stmts[block_idx].kind {
            StmtKind::Let(_local) => {
                return block_idx + 1;
            }
            StmtKind::Item(_item) => {
                return block_idx + 1;
            }
            StmtKind::Expr(no_semi_expr) => match &no_semi_expr.kind {
                // Blocks.
                // recurse on nested block,
                // but we still only instrumented one (block) stmt, so just
                // move to the next stmt (return i+1).
                ExprKind::Block(block, _) => {
                    self.block_to_decls(
                        ppt_name,
                        block,
                        param_decls,
                        &param_to_block_idx,
                        &ret_ty,
                        exit_counter,
                    );
                    return block_idx + 1;
                }
                ExprKind::If(_, if_block, None) => {
                    // no else.
                    self.block_to_decls(
                        ppt_name,
                        if_block,
                        param_decls,
                        &param_to_block_idx,
                        &ret_ty,
                        exit_counter,
                    );
                    return block_idx + 1;
                }
                ExprKind::If(_, if_block, Some(expr)) => {
                    // yes else.
                    self.block_to_decls(
                        ppt_name,
                        if_block,
                        param_decls,
                        &param_to_block_idx,
                        &ret_ty,
                        exit_counter,
                    );

                    self.if_stmt_to_decls(
                        expr,
                        exit_counter,
                        ppt_name,
                        param_decls,
                        &param_to_block_idx,
                        &ret_ty,
                    );
                    return block_idx + 1;
                }
                ExprKind::While(_, while_block, _) => {
                    self.block_to_decls(
                        ppt_name,
                        while_block,
                        param_decls,
                        &param_to_block_idx,
                        &ret_ty,
                        exit_counter,
                    );
                    return block_idx + 1;
                }
                ExprKind::ForLoop {
                    pat: _,
                    iter: _,
                    body: for_block,
                    label: _,
                    kind: _,
                } => {
                    self.block_to_decls(
                        ppt_name,
                        for_block,
                        param_decls,
                        &param_to_block_idx,
                        &ret_ty,
                        exit_counter,
                    );
                    return block_idx + 1;
                }
                ExprKind::Loop(loop_block, _, _) => {
                    self.block_to_decls(
                        ppt_name,
                        loop_block,
                        param_decls,
                        &param_to_block_idx,
                        &ret_ty,
                        exit_counter,
                    );
                    return block_idx + 1;
                } // missing Match blocks, TryBlock, Const block? probably more.
                _ => {}
            },
            // Look for returns. dtrace passes have run, so all exit points should
            // be identifiable by an explicit return stmt.
            StmtKind::Semi(semi) => match &semi.kind {
                ExprKind::Ret(None) => {
                    write_exit(ppt_name, *exit_counter);
                    *exit_counter += 1;
                    for idx in 0..param_decls.len() {
                        param_decls[idx].write();
                        //write(&mut param_decls[idx], "", true, false, "", &mut None);
                    }
                    write_newline();

                    // we're sitting on the void return we just processed, so inc
                    // to move on.
                    block_idx += 1;
                }
                ExprKind::Ret(Some(_)) => {
                    write_exit(ppt_name, *exit_counter);
                    *exit_counter += 1;
                    for idx in 0..param_decls.len() {
                        param_decls[idx].write();
                        //write(&mut param_decls[idx], "", true, false, "", &mut None);
                    }

                    // make return TopLevlDecl.
                    match &ret_ty {
                        FnRetTy::Default(_) => {} // no return record to be had.
                        FnRetTy::Ty(ty) => {
                            let var_name = String::from("return");
                            let mut is_ref = false;
                            let mut write_p = true;
                            let mut return_decl = match &get_rep_type(&ty.kind, &mut is_ref, None) {
                                RepType::Prim(p_type) => {
                                    TopLevlDecl {
                                        map: self.map,
                                        var_name: var_name.clone(),
                                        dec_type: p_type.clone(),
                                        rep_type: p_type.clone(),
                                        struct_name: None,
                                        field_decls: Vec::new(),
                                        contents: None,
                                    } // Ready to write this var decl.
                                }
                                RepType::HashCodeStruct(ty_string) => {
                                    // TOOD: remove this.
                                    // write_p = !ty_string.starts_with("Option") && !ty_string.starts_with("Result");
                                    // println!("write_p is {} for {}", write_p, ty_string);
                                    let mut tmp = TopLevlDecl {
                                        map: self.map,
                                        var_name: var_name.clone(),
                                        dec_type: ty_string.clone(),
                                        rep_type: String::from("hashcode"),
                                        struct_name: Some(ty_string.clone()),
                                        field_decls: Vec::new(),
                                        contents: None,
                                    };
                                    tmp.construct_field_decls(self.depth_limit, &mut write_p);

                                    // Error checking.
                                    if !write_p {
                                        // Any "fields" are invalid, but tmp could be an enum/union and pointer is valid.
                                        match &tmp.contents {
                                            None => {
                                                for j in 0..tmp.field_decls.len() {
                                                    tmp.field_decls[j].decl.var_name =
                                                        String::from("false");
                                                }
                                            }
                                            Some(_) => panic!("Expected some field_decls"),
                                        }
                                    }
                                    if ty_string.starts_with("Option")
                                        || ty_string.starts_with("Result")
                                    {
                                        // this record is also invalid.
                                        tmp.var_name = String::from("false");
                                    }
                                    tmp
                                }
                                RepType::PrimArray(p_type) => {
                                    TopLevlDecl {
                                        map: self.map,
                                        var_name: var_name.clone(),
                                        dec_type: format!("{}[]", p_type),
                                        rep_type: String::from("hashcode"),
                                        struct_name: None,
                                        field_decls: Vec::new(),
                                        contents: Some(Box::new(ArrayContents {
                                            decl: Box::new(TopLevlDecl {
                                                map: self.map,
                                                var_name: format!("{}[..]", var_name),
                                                dec_type: format!("{}[]", p_type),
                                                rep_type: format!("{}[]", p_type),
                                                struct_name: None,
                                                field_decls: Vec::new(),
                                                contents: None,
                                            }),
                                            enclosing_var: var_name.clone(),
                                            sub_contents: None,
                                        })), // Ready to write this var_decl.
                                    }
                                }
                                RepType::HashCodeArray(ty_string) => {
                                    let mut tmp = TopLevlDecl {
                                        map: self.map,
                                        var_name: var_name.clone(),
                                        dec_type: format!("{}[]", ty_string),
                                        rep_type: String::from("hashcode"),
                                        struct_name: Some(ty_string.clone()),
                                        field_decls: Vec::new(),
                                        contents: Some(Box::new(ArrayContents {
                                            decl: Box::new(TopLevlDecl {
                                                map: self.map,
                                                var_name: format!("{}[..]", var_name),
                                                dec_type: format!("{}[]", ty_string),
                                                rep_type: String::from("hashcode[]"),
                                                struct_name: Some(ty_string.clone()),
                                                field_decls: Vec::new(),
                                                contents: None,
                                            }),
                                            enclosing_var: var_name.clone(),
                                            sub_contents: Some(Vec::new()),
                                        })),
                                    };
                                    match &mut tmp.contents {
                                        None => panic!("Missing contents in HashCodeArray"),
                                        Some(contents) => {
                                            contents
                                                .build_contents(self.depth_limit - 1, &mut write_p);

                                            // Error checking.
                                            if !write_p {
                                                // Any "fields" are invalid, but tmp could be an enum/union and pointers is valid.
                                                match &mut contents.sub_contents {
                                                    None => panic!("Expected some field_decls 1"),
                                                    Some(sub_contents) => {
                                                        for j in 0..sub_contents.len() {
                                                            sub_contents[j].decl.var_name =
                                                                String::from("false");
                                                        }
                                                    }
                                                }
                                            }

                                            if ty_string.starts_with("Option")
                                                || ty_string.starts_with("Result")
                                            {
                                                // this record is also invalid.
                                                tmp.var_name = String::from("false");
                                            }
                                        }
                                    }
                                    tmp
                                }
                                RepType::Skip => panic!("What TopLevlDecl should we produce here"),
                            };
                            return_decl.write();
                            //write(&mut Box::new(return_decl), "", true, false, "", &mut None);
                        }
                    }

                    write_newline();
                    // probably:
                    block_idx += 1;
                }
                ExprKind::Call(_call, _params) => {
                    return block_idx + 1;
                } // Maybe check for drop and other invalidations.
                _ => {
                    return block_idx + 1;
                } // other things you overlooked.
            },
            // FIXME: remove this.
            // StmtKind::Expr(no_semi_expr) => match &no_semi_expr.kind {
            //     ExprKind::Match(..) => {
            //         return i + 1;
            //     }
            //     _ => panic!("is this non-semi expr a return or a valid non-semi expr?"),
            // },
            _ => {
                return block_idx + 1;
            }
        }
        block_idx
    }

    // Walk a new block looking for exit points for writing exit-ppt declarations
    // and nested blocks to recursively look for exit points.
    // See rustc_parse::parser::item::instrument_block.
    // * `ppt_name` - The ppt name.
    // * `body` - Block to check for program points.
    // * `param_decls` - Vec of TopLevlDecl representing data
    //                    needed to write variable declarations to the
    //                    decls file at program points.
    // * `param_to_block_idx` - Map of param identifiers to idx into
    //                          dtrace_param_blocks.
    // * `ret_ty` - Return type of the function.
    // * `exit_counter` - Gives the previously seen number of exit ppts.
    fn block_to_decls(
        &mut self,
        ppt_name: &str,
        body: &Box<Block>,
        param_decls: &mut Vec<Box<TopLevlDecl<'_>>>,
        param_to_block_idx: &FxHashMap<String, i32>,
        ret_ty: &FnRetTy,
        exit_counter: &mut usize,
    ) {
        let mut i = 0;

        // assuming no unreachable statements.
        while i < body.stmts.len() {
            // make sure loop bound is growing as we insert stmts.
            i = self.stmt_to_decls(
                i,
                body,
                exit_counter,
                ppt_name,
                param_decls,
                &param_to_block_idx,
                &ret_ty,
            ); // match on Semi and blocks mainly for now, find return <expr>; and add an exit point.
        }
    }

    // is it a good idea to store which params are valid at each exit
    // ppt for the decls pass which happens after this?
    // then the decls pass just needs to:
    // 1: visit_item to build FxHashMap<ident, StructNode>.
    // 2: visit_fn, grok sig, and grok exit ppts using structural
    //    recursion on StructNodes for nesting. Need to use depth counter
    //    for a base case.

    // Walk a function body looking for exit points.
    // See rustc_parse::parser::item::instrument_fn_body.
    // * `ppt_name` - The ppt name.
    // * `body` - Block to check for program points.
    // * `param_decls` - Vec of TopLevlDecl representing data
    //                    needed to write variable declarations to the
    //                    decls file at program points.
    // * `param_to_block_idx` - Map of param identifiers to idx into
    //                          dtrace_param_blocks.
    // * `ret_ty` - Return type of the function.
    fn fn_body_to_decls(
        &mut self,
        ppt_name: &str,
        body: &Box<Block>,
        param_decls: &mut Vec<Box<TopLevlDecl<'_>>>,
        param_to_block_idx: FxHashMap<String, i32>,
        ret_ty: &FnRetTy,
    ) {
        // look for returns and nested blocks (recurse in those cases).
        let mut exit_counter = 1;

        // assuming no unreachable statements.
        let mut i = 0;
        while i < body.stmts.len() {
            i = self.stmt_to_decls(
                i,
                body,
                &mut exit_counter,
                ppt_name,
                param_decls,
                &param_to_block_idx,
                &ret_ty,
            );
        }
    }
}

// Process a function signature and build up a new Vec<TopLevlDecl>
// ready to be subsequently written to the decls file before we
// walk the function body looking for exit points. Each TopLevlDecl
// represents a parameter and information necessary to write a variable
// declaration for the parameter.
// See rustc_parse::parser::item::fn_sig_to_dtrace_code.
// * `decl` - Function declaration of the function being processed.
// * `map` - Retrieve Item from struct identifier.
// * `depth_limit` - Argument to limit writing recursively defined structs
//                   to the decls file.
fn fn_sig_to_toplevl_decls<'a>(
    decl: &'a Box<FnDecl>,
    map: &'a FxHashMap<String, Box<Item>>,
    depth_limit: u32,
    visitor: Option<&DaikonDeclsVisitor<'_>>,
) -> Vec<Box<TopLevlDecl<'a>>> {
    let mut var_decls: Vec<Box<TopLevlDecl<'_>>> = Vec::new();
    for i in 0..decl.inputs.len() {
        let var_name = get_param_ident(&decl.inputs[i].pat);
        let mut is_ref = false;
        let mut write_p = true;
        let toplevl_decl = match &get_rep_type(&decl.inputs[i].ty.kind, &mut is_ref, visitor) {
            RepType::Prim(p_type) => {
                TopLevlDecl {
                    map,
                    var_name: var_name.clone(),
                    dec_type: p_type.clone(),
                    rep_type: p_type.clone(),
                    struct_name: None,
                    field_decls: Vec::new(),
                    contents: None,
                } // Ready to write this var decl.
            }
            RepType::HashCodeStruct(ty_string) => {
                let mut decl = TopLevlDecl {
                    map,
                    var_name: var_name.clone(),
                    dec_type: ty_string.clone(),
                    rep_type: String::from("hashcode"),
                    struct_name: Some(ty_string.clone()),
                    field_decls: Vec::new(),
                    contents: None,
                };
                decl.construct_field_decls(depth_limit, &mut write_p);

                // Error checking.
                if !write_p {
                    // Any "fields" are invalid, but decl could be an enum/union and pointer is valid.
                    match &decl.contents {
                        None => {
                            for j in 0..decl.field_decls.len() {
                                decl.field_decls[j].decl.var_name = String::from("false");
                            }
                        }
                        Some(_) => panic!("Expected some field_decls"),
                    }
                }
                if ty_string.starts_with("Option") || ty_string.starts_with("Result") {
                    // this record is also invalid.
                    decl.var_name = String::from("false");
                }
                decl
            }
            RepType::PrimArray(p_type) => {
                TopLevlDecl {
                    map,
                    var_name: var_name.clone(),
                    dec_type: format!("{}[]", p_type),
                    rep_type: String::from("hashcode"),
                    struct_name: None,
                    field_decls: Vec::new(),
                    contents: Some(Box::new(ArrayContents {
                        decl: Box::new(TopLevlDecl {
                            map,
                            var_name: format!("{}[..]", var_name),
                            dec_type: format!("{}[]", p_type),
                            rep_type: format!("{}[]", p_type),
                            struct_name: None,
                            field_decls: Vec::new(),
                            contents: None,
                        }),
                        enclosing_var: var_name.clone(),
                        sub_contents: None,
                    })), // Ready to write this var_decl.
                }
            }
            RepType::HashCodeArray(ty_string) => {
                let mut tmp = TopLevlDecl {
                    map,
                    var_name: var_name.clone(),
                    dec_type: format!("{}[]", ty_string),
                    rep_type: String::from("hashcode"),
                    struct_name: Some(ty_string.clone()),
                    field_decls: Vec::new(),
                    contents: Some(Box::new(ArrayContents {
                        decl: Box::new(TopLevlDecl {
                            map,
                            var_name: format!("{}[..]", var_name),
                            dec_type: format!("{}[]", ty_string),
                            rep_type: String::from("hashcode[]"),
                            struct_name: Some(ty_string.clone()),
                            field_decls: Vec::new(),
                            contents: None,
                        }),
                        enclosing_var: var_name.clone(),
                        sub_contents: Some(Vec::new()),
                    })),
                };
                match &mut tmp.contents {
                    None => panic!("Missing contents in HashCodeArray"),
                    Some(contents) => {
                        contents.build_contents(depth_limit - 1, &mut write_p);

                        // Error checking: note for this and similar, tmp.contents valid is equivalent to tmp valid, if we have Vec of enums, contents is pointers.
                        if !write_p {
                            // Any "fields" are invalid, but tmp could be an enum/union and pointers is valid.
                            match &mut contents.sub_contents {
                                None => panic!("Expected some field_decls 1"),
                                Some(sub_contents) => {
                                    for j in 0..sub_contents.len() {
                                        sub_contents[j].decl.var_name = String::from("false");
                                    }
                                }
                            }
                        }

                        if ty_string.starts_with("Option") || ty_string.starts_with("Result") {
                            // this record is also invalid.
                            tmp.var_name = String::from("false");
                        }
                    }
                }
                tmp
            }
            RepType::Skip => {
                continue;
            }
        };
        var_decls.push(Box::new(toplevl_decl));
    }

    var_decls
}

impl<'a> Visitor<'a> for DaikonDeclsVisitor<'a> {
    // Process a new function and write it to the decls file.
    // * `fk` - The function kind, i.e., function or closure.
    fn visit_fn(
        &mut self,
        fk: FnKind<'a>,
        _attrs: &rustc_ast::AttrVec,
        _span: rustc_span::Span,
        _id: rustc_ast::NodeId,
    ) {
        match &fk {
            FnKind::Fn(_, _, f) => {
                if !f.generics.params.is_empty() {
                    // Skip generics for now.
                    return;
                }
                if !f.ident.as_str().starts_with("dtrace") {
                    let ppt_name = f.ident.as_str();
                    write_entry(ppt_name);
                    let param_to_block_idx = map_params(&f.sig.decl);
                    let mut param_decls = fn_sig_to_toplevl_decls(
                        &f.sig.decl,
                        self.map,
                        self.depth_limit,
                        Some(&self),
                    );
                    for i in 0..param_decls.len() {
                        param_decls[i].write();
                        //write(&mut param_decls[i], "", true, false, "", &mut None);
                    }
                    write_newline();
                    match &f.body {
                        None => {}
                        Some(body) => {
                            // By now, all exit ppts are
                            // explicit Semi(Ret) stmts.
                            self.fn_body_to_decls(
                                ppt_name,
                                body,
                                &mut param_decls,
                                param_to_block_idx,
                                &f.sig.decl.output,
                            );
                        }
                    }
                }
            }
            _ => {}
        }
        visit::walk_fn(self, fk);
    }

    // Look for impl blocks. Also skip inline mods.
    fn visit_item(&mut self, item: &'a Item) {
        let mut inline_mod_p = false;
        let mut do_pop = false;

        match &item.kind {
            ItemKind::Mod(_, _, kind) => match &kind {
                ModKind::Loaded(_, inline, _) => match &inline {
                    Inline::Yes => {
                        inline_mod_p = true;
                    }
                    _ => {}
                },
                _ => {}
            },
            ItemKind::Impl(imp) => match &imp.self_ty.kind {
                TyKind::Path(_, path) => {
                    let plain_struct = String::from(path.segments[0].ident.as_str());
                    self.push_struct(plain_struct);
                    do_pop = true;
                }
                _ => {}
            },
            // TODO: skip generic types
            /* ItemKind::Struct(_, generics, variant_data) => match variant_data {
                VariantData::Struct { fields: _, recovered: _ } => {
                    for i in 0..generics.params.len() {
                        match &generics.params[i].kind {
                            GenericParamKind::Type { default: _ } => {
                                // Skip generic types for now.
                                return;
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }, */
            _ => {}
        };

        if !inline_mod_p {
            visit::walk_item(self, item);
        }

        if do_pop {
            self.pop_struct();
        }
    }
}

// Lock on the decls file.
static DECLS: LazyLock<Mutex<Option<std::fs::File>>> = LazyLock::new(|| Mutex::new(decls_open()));

// Open the decls file.
fn decls_open() -> Option<std::fs::File> {
    let decls_path = format!("{}{}", *OUTPUT_PREFIX.lock().unwrap(), ".decls");
    let decls = std::path::Path::new(&decls_path);
    Some(
        std::fs::File::options()
            .write(true)
            .append(true)
            .open(&decls)
            .unwrap(),
    )
}

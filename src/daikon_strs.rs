// FIXME: add one test for each String.

use rustc_data_structures::fx::FxHashMap;

use crate::dtrace_visitor::OUTPUT_PREFIX;

// Proper primitive types
// i8, i16, i32, i64, i128 and isize
// u8, u16, u32, u64, u128 and usize
// f32, f64
// char
// bool
// () -- why is this possible for parameters :/

pub static I8: &str = "i8";
pub static I16: &str = "i16";
pub static I32: &str = "i32";
pub static I64: &str = "i64";
pub static I128: &str = "i128";
pub static ISIZE: &str = "isize";

pub static U8: &str = "u8";
pub static U16: &str = "u16";
pub static U32: &str = "u32";
pub static U64: &str = "u64";
pub static U128: &str = "u128";
pub static USIZE: &str = "usize";

pub static F32: &str = "f32";
pub static F64: &str = "f64";

pub static CHAR: &str = "char";
pub static BOOL: &str = "bool";
pub static UNIT: &str = "()";
pub static STR: &str = "str";
pub static STRING: &str = "String";
pub static VEC: &str = "Vec";

// Iteration order is irrelevant, we can substitute in any order.
#[allow(rustc::potential_query_instability)]
pub(crate) fn substitute(replacements: FxHashMap<&str, &str>, string: &str) -> String {
    let mut finished = String::from(string);
    for (key, value) in replacements.iter() {
        finished = finished.replace(key, value);
    }

    finished
}

// Initialize a function-local nonce counter.
pub(crate) static DTRACE_INIT_NONCE: &str = "fn main() { let mut __daikon_nonce = 0;
let mut __unwrap_nonce = NONCE_COUNTER.lock().unwrap();
__daikon_nonce = *__unwrap_nonce;
*__unwrap_nonce += 1;
drop(__unwrap_nonce);
 }";

// Build dtrace entry call for each ppt-enter.
// program_point: ppt-name (e.g. MAIN, FOO)
/* Ex:

  fn foo() {
      dtrace_entry("FOO:::ENTER", __daikon_nonce);
  }

*/
pub(crate) static DTRACE_ENTRY: &str =
    "fn foo() { dtrace_entry(\"${program_point}:::ENTER\", __daikon_nonce); }";

// Build dtrace exit call for each ppt-exit.
// ${program_point}: ppt-name.
// ${exit_num}: number corresponding to this exit ppt, unique for each within the function.
/*

  fn foo() {
      dtrace_exit("FOO:::EXIT2", __daikon_nonce);
  }


*/
pub(crate) static DTRACE_EXIT: &str =
    "fn foo() { dtrace_exit(\"${program_point}:::EXIT${exit_num}\", __daikon_nonce); }";

// Build a call to log a primitive value.
// ${prim_type}: Primitive type of the variable.
// ${variable_name}: var identifier.
/* Ex:

  fn foo() {
      dtrace_print_prim::<i32>(parameter1, String::from("parameter1"));
  }

*/
pub(crate) static DTRACE_PRIM: &str = "fn foo() { dtrace_print_prim::<${prim_type}>(${variable_name}, String::from(\"${variable_name}\")); }";

// Build a log statement for a primitive return value.
// ${prim_type}: Primitive type of the return value.
/* Ex:

  fn foo() {
      dtrace_print_prim::<i32>(__daikon_ret, String::from("return"));
  }

*/
pub(crate) static DTRACE_PRIM_RET: &str =
    "fn foo() { dtrace_print_prim::<${prim_type}>(__daikon_ret, String::from(\"return\")); }";

// Build log stmt for return variable with primitive reference type (see below).
// ${prim_type}: Primitive type of the variable.
/* Ex:

  fn foo() {
      dtrace_print_prim::<i32>(i32::from_str(&__daikon_ret.to_string()).expect("Variable parsing failed"), String::from("return"));
  }

*/
pub(crate) static DTRACE_PRIM_REF_RET: &str = "fn foo() { dtrace_print_prim::<${prim_type}>(${prim_type}::from_str(&__daikon_ret.to_string()).expect(\"Variable parsing failed\"), String::from(\"return\")); }";

// Build log statement for a variable with primitive reference type.
// ${prim_type}: Primitive type of the variable.
// ${variable_name}: Identifier of the variable.
/*

  fn foo() {
      dtrace_print_prim::<i32>(i32::from_str(&parameter1.to_string()).expect("Variable parsing failed"), String::from("parameter1"));
  }

*/
pub(crate) static DTRACE_PRIM_REF: &str = "fn foo() { dtrace_print_prim::<${prim_type}>(${prim_type}::from_str(&${variable_name}.to_string()).expect(\"Variable parsing failed\"), String::from(\"${variable_name}\")); }";

// Build log statement for return variable of String-like type (&str, etc.).
/*

  fn foo() {
      dtrace_print_string(__daikon_ret.to_string(), String::from("return"));
  }

*/
pub(crate) static DTRACE_PRIM_TOSTRING_RET: &str =
    "fn foo() { dtrace_print_string(__daikon_ret.to_string(), String::from(\"return\")); }";

// Build log statement for variable of String-like type (&str, etc.).
// ${variable_name}: Identifier for variable.
/*

  fn foo() {
      dtrace_print_string(parameter1.to_string(), String::from("parameter1"));
  }

*/
pub(crate) static DTRACE_PRIM_TOSTRING: &str = "fn foo() { dtrace_print_string(${variable_name}.to_string(), String::from(\"${variable_name}\")); }";

// Build log stmt for a String field.
// ${field_name}: Field name.
/* Ex:

  dtrace_print_string(self.field1.to_string(), format!("{}{}", prefix, ".field1"));

*/
pub(crate) static DTRACE_PRIM_FIELD_TOSTRING: &str = "dtrace_print_string(self.${field_name}.to_string(), format!(\"{}{}\", prefix, \".${field_name}\"));";

// Build log stmt for primitive field (non-string type).
// ${prim_type}: Primitive type of field.
// ${field_name}: Field name.
/*

  dtrace_print_prim::<i32>(self.field1, format!("{}{}", prefix, ".field1"));

*/
pub(crate) static DTRACE_PRIM_STRUCT: &str = "dtrace_print_prim::<${prim_type}>(self.${field_name}, format!(\"{}{}\", prefix, \".${field_name}\"));";

// FIXME: if you have Vec<&'a &'b i32>, you will probably have to make a new Vec<i32> like this
//       to satisfy dtrace_print_prim_vec<T>(v: &Vec<T>).

// Build log stmt for fields with primitive reference type.
// ${prim_type}: Underlying primitive type of field.
// ${field_name}: Field name.
/* Ex:

  dtrace_print_prim::<i32>(i32::from_str(&self.field1.to_string()).expect("Variable parsing failed"), format!("{}{}", prefix, ".field1"));

*/
pub(crate) static DTRACE_PRIM_REF_STRUCT: &str = "dtrace_print_prim::<${prim_type}>(${prim_type}::from_str(&self.${field_name}.to_string()).expect(\"Ok\"), format!(\"{}{}\", prefix, \".${field_name}\"));";

// Build log stmt for a struct variable.
// ${variable_name}: Variable name.
/* Ex:

  fn foo() {
      dtrace_print_pointer(parameter1 as *const _ as usize, String::from("parameter1"));
      parameter1.dtrace_print_fields(3, String::from("parameter1"));
  }

*/
pub(crate) static DTRACE_USERDEF: &str =
    "fn foo() { dtrace_print_pointer(${variable_name} as *const _ as usize, String::from(\"${variable_name}\"));
 ${variable_name}.dtrace_print_fields(3, String::from(\"${variable_name}\")); }";

// Build log stmt for struct variable, syntax difference.
// ${variable_name}: Variable name.
/* Ex:

  fn foo() {
      dtrace_print_pointer(&parameter1 as *const _ as usize, String::from("parameter1"));
      parameter1.dtrace_print_fields(3, String::from("parameter1"));
  }

*/
pub(crate) static DTRACE_USERDEF_AMPERSAND: &str =
    "fn foo() { dtrace_print_pointer(&${variable_name} as *const _ as usize, String::from(\"${variable_name}\"));
 ${variable_name}.dtrace_print_fields(3, String::from(\"${variable_name}\")); }";

/* Ex: same as below, with no ampersand access.

  fn foo() {
      dtrace_print_pointer(__daikon_ret as *const _ as usize, String::from("return"));
      __daikon_ret.dtrace_print_fields(3, String::from("return"));
  }

*/
pub(crate) static DTRACE_USERDEF_RET: &str =
    "fn foo() { dtrace_print_pointer(__daikon_ret as *const _ as usize, String::from(\"return\"));
__daikon_ret.dtrace_print_fields(3, String::from(\"return\")); }";

/* Ex: log the return variable when it is a struct.
 * Note: This was configured to use variable depth arg, we can worry about that later
   for all the strings.

  fn foo() {
      dtrace_print_pointer(&__daikon_ret as *const _ as usize, String::from("return"));
      __daikon_ret.dtrace_print_fields(3, String::from("return"));
  }

*/
pub(crate) static DTRACE_USERDEF_RET_AMPERSAND: &str = "fn foo() {
      dtrace_print_pointer(&__daikon_ret as *const _ as usize, String::from(\"return\"));
      __daikon_ret.dtrace_print_fields(3, String::from(\"return\"));
  }";

// ${field_name}: Field name.
/* Ex: same as below but no ampersand access.

  dtrace_print_pointer(self.{field1} as *const _ as usize, format!("{}{}", prefix, ".{field1}"));
  self.{field1}.dtrace_print_fields(depth - 1, format!("{}{}", prefix, ".{field1}"));

*/
pub(crate) static DTRACE_USERDEF_STRUCT: &str =
  "dtrace_print_pointer(self.${field_name} as *const _ as usize, format!(\"{}{}\", prefix, \".${field_name}\"));
  self.${field_name}.dtrace_print_fields(depth - 1, format!(\"{}{}\", prefix, \".${field_name}\"));";

// ${field_name}: Field name.
/* Ex: We have a type X with field {field1}, and we want to recurse on {field1} and print its
   pointer value and fields.

  dtrace_print_pointer(&self.{field1} as *const _ as usize, format!("{}{}", prefix, ".{field1}"));
  self.{field1}.dtrace_print_fields(depth - 1, format!("{}{}", prefix, ".{field1}"));

*/
pub(crate) static DTRACE_USERDEF_STRUCT_AMPERSAND: &str =
  "dtrace_print_pointer(&self.${field_name} as *const _ as usize, format!(\"{}{}\", prefix, \".${field_name}\"));
  self.${field_name}.dtrace_print_fields(depth - 1, format!(\"{}{}\", prefix, \".${field_name}\"));";

// always with ampersand, we will always make a copy.
// ${type}: The struct/enum/union type.
// ${vec_name}: Name of the temporary Vec.
// ${field_name}: Name of the field with type Vec we want to print.
/* Ex: We have a temporary Vec<{X}> that has the values of X.{field1}, and
   we want to recurse and print the values of Vec<{X.*}>.

  {X}::dtrace_print_fields_vec(&{__daikon_tmp1}, depth - 1, format!("{}{}", prefix, ".{field1}"));

*/
pub(crate) static DTRACE_USERDEF_VEC_FIELDS: &str = "${type}::dtrace_print_fields_vec(&${vec_name}, depth - 1, format!(\"{}{}\", prefix, \".${field_name}\"));";

// ${type}: The struct/enum/union type.
// ${field_name}: Field name.
// ${vec_name}: Name of the temporary Vec.
/* Ex: We have a temporary Vec<{X}>, where type {X} has field {field1},
   and we want to print the values of Vec<{X.field1}>.

  {X}::dtrace_print_{field1}_vec(&{__daikon_tmp1}, format!("{}{}", prefix, ".{field1}"));

*/
pub(crate) static DTRACE_PRINT_XFIELD_VEC: &str = "${type}::dtrace_print_${field_name}_vec(&${vec_name}, format!(\"{}{}\", prefix, \".${field_name}\"));";

// ${type}: Struct/enum/union type.
// ${field_name}: Field name.
// ${vec_name}: Name of the temporary Vec.
/* Ex: We have a Vec<Y>, and we have a copy of the fields Vec<Y.{field1}>, and we
   want to print the pointer values for the latter.

  dtrace_print_pointer_vec::<{X}>(&{__daikon_tmp1}, format!("{}{}", prefix, ".{field1}"));

*/
pub(crate) static DTRACE_PRINT_POINTER_VEC_USERDEF: &str = "dtrace_print_pointer_vec::<${type}>(&${vec_name}, format!(\"{}{}\", prefix, \".${field_name}\"));";

// we're expecting a tmp vec loop before this.
// ${variable_name}: Variable name.
/*

  dtrace_print_pointer({parameter1}.as_ptr() as usize, String::from("{parameter1}"));

*/
pub(crate) static DTRACE_VEC_POINTER: &str =
    "dtrace_print_pointer(${variable_name}.as_ptr() as usize, String::from(\"${variable_name}\"));";

/* Ex: Function returns a Vec; we want to print the return variable pointer
   value.

  dtrace_print_pointer(__daikon_ret.as_ptr() as usize, String::from("return"));

*/
pub(crate) static DTRACE_VEC_POINTER_RET: &str =
    "dtrace_print_pointer(__daikon_ret.as_ptr() as usize, String::from(\"return\"));";

// ${type}: Struct/enum/union type.
// ${variable_name}: Variable name.
// ${vec_name}: Name of the temporary Vec.
/*

  dtrace_print_pointer_vec::<{X}>(&{__daikon_tmp1}, format!("{}{}", String::from("{parameter1}"), "[..]"));

*/
pub(crate) static DTRACE_PRINT_POINTER_VEC: &str = "dtrace_print_pointer_vec::<${type}>(&${vec_name}, format!(\"{}{}\", String::from(\"${variable_name}\"), \"[..]\"));";

// ${type}: Struct/enum/union type.
// ${variable_name}: Variable name.
// ${vec_name}: Name of the temporary Vec.
/* Ex: {parameter1} is a Vec<{X}> and we want to initiate printing its contents.

      {X}::dtrace_print_fields_vec(&{__daikon_tmp1}, 3, format!("{}{}", String::from("{parameter1}"), "[..]"));
  }

*/
pub(crate) static DTRACE_VEC_FIELDS: &str =
      "${type}::dtrace_print_fields_vec(&${vec_name}, 3, format!(\"{}{}\", String::from(\"${variable_name}\"), \"[..]\"));
  }";

// ${type}: Struct/enum/union type.
// ${counter_name}: Name of anonymous counter.
// ${vec_name}: Name of anonymous Vec.
/* Ex: copy a synthesized v Vec into a new temporary.

  let mut {__daikon_tmp1}: Vec<&{X}> = Vec::new();
  for {__daikon_tmp2} in 0..v.len() {
      {__daikon_tmp1}.push(v[{__daikon_tmp2}]);
  }

*/
pub(crate) static DTRACE_TMP_VEC_USERDEF: &str = "let mut ${vec_name}: Vec<&${type}> = Vec::new();
  for ${counter_name} in 0..v.len() {
      ${vec_name}.push(v[${counter_name}]);
  }";

// ${type}: Struct/enum/union type.
// ${counter_name}: Name of anonymous counter.
// ${vec_name}: Name of anonymous Vec.
// ${field_name}: Field name.
/* Ex: like below, but no ampersand access.

  let mut {__daikon_tmp1}: Vec<&{X}> = Vec::new();
  for {__daikon_tmp2} in 0..v.len() {
      {__daikon_tmp1}.push(v[{__daikon_tmp2}].{field1});
  }

*/
pub(crate) static DTRACE_TMP_VEC_USERDEF_FIELD: &str =
    "let mut ${vec_name}: Vec<&${type}> = Vec::new();
  for ${counter_name} in 0..v.len() {
      ${vec_name}.push(v[${counter_name}].${field_name});
  }";

// ${type}: Struct/enum/union type.
// ${counter_name}: Name of anonymous counter.
// ${vec_name}: Name of anonymous Vec.
// ${field_name}: Field name.
/* Ex: we have access to a synthetic variable v, and
   it contains structs of type Y, with fields Y::{field1}
   of type {X}, and we want to copy over the contents of
   {field1} into a new temporary.

  let mut {__daikon_tmp1}: Vec<&{X}> = Vec::new();
  for {__daikon_tmp2} in 0..v.len() {
      {__daikon_tmp1}.push(&v[{__daikon_tmp2}].{field1});
  }

*/
pub(crate) static DTRACE_TMP_VEC_USERDEF_FIELD_AMPERSAND: &str =
    "let mut {__daikon_tmp1}: Vec<&{X}> = Vec::new();
  for {__daikon_tmp2} in 0..v.len() {
      {__daikon_tmp1}.push(&v[{__daikon_tmp2}].{field1});
  }";

// ${type}: Struct/enum/union type.
// ${counter_name}: Name of anonymous counter.
// ${vec_name}: Name of anonymous Vec.
// ${variable_name}: Variable name.
// this will always be mashed with some subsequent call, so don't close __skip yet.
// the thing you mash it with must close __skip.
/* Ex: like below, but we don't need ampersand access on the Vec elements.

  fn foo() {
      let mut {__daikon_tmp1}: Vec<&{X}> = Vec::new();
      for {__daikon_tmp2} in 0..{parameter1}.len() {
          {__daikon_tmp1}.push({parameter1}[{__daikon_tmp2}]);
      }

*/
pub(crate) static DTRACE_TMP_VEC: &str = "fn foo() {
      let mut ${vec_name}: Vec<&${type}> = Vec::new();
      for ${counter_name} in 0..${variable_name}.len() {
          ${vec_name}.push(${variable_name}[${counter_name}]);
      }";

// ${type}: Struct/enum/union type
// ${vec_name}: Name of anonymous Vec.
// ${counter_name}: Name of anonymous counter.
// ${variable_name}: Variable name.
// FIXME: use this for params/returns where you have Vec<Type>.
/* Ex: {parameter1} is a Vec<{X}>, and we want to copy over the
   contents into a temporary.

  fn foo() {
      let mut {__daikon_tmp1}: Vec<&{X}> = Vec::new();
      for {__daikon_tmp2} in 0..{parameter1}.len() {
          {__daikon_tmp1}.push(&{parameter1}[{__daikon_tmp2}]);
      }

*/
pub(crate) static DTRACE_TMP_VEC_AMPERSAND: &str = "fn foo() {
      let mut ${vec_name}: Vec<&${type}> = Vec::new();
      for ${counter_name} in 0..${variable_name}.len() {
          ${vec_name}.push(&${variable_name}[${counter_name}]);
      }";

// ${prim_type}: Primitive type.
// ${vec_name}: Name of anonymous Vec.
// ${counter_name}: Name of anonymous counter.
// ${variable_name}: Variable name.
/* Ex: Copy over the contents of {parameter1}, a primitive vec/array type into a temporary.

  fn foo() {
      let mut {__daikon_tmp1}: Vec<{i32}> = Vec::new();
      for {__daikon_tmp2} in 0..{parameter1}.len() {
          {__daikon_tmp1}.push({i32}::from_str(&{parameter1}[{__daikon_tmp2}].to_string()).expect("Parsing failed"));
      }

*/
pub(crate) static DTRACE_TMP_VEC_PRIM: &str =
  "fn foo() {
      let mut ${vec_name}: Vec<${prim_type}> = Vec::new();
      for ${counter_name} in 0..${variable_name}.len() {
          ${vec_name}.push(${prim_type}::from_str(&${variable_name}[${counter_name}].to_string()).expect(\"Variable parsing failed\"));
      }";

// ${type}: Struct/enum/union type.
// ${vec_name}: Name of anonymous Vec.
// ${counter_name}: Name of anonymous counter.
// ${field_name}: Field name.
/* Ex: same as below, but no ampsersand access for self.

  let mut {__daikon_tmp1}: Vec<&{X}> = Vec::new();
  for {__daikon_tmp2} in 0..self.{field1}.len() {
      {__daikon_tmp1}.push(self.{field1}[{__daikon_tmp2}]);
  }


*/
pub(crate) static DTRACE_TMP_VEC_FOR_FIELD: &str =
    "let mut ${vec_name}: Vec<&${type}> = Vec::new();
  for ${counter_name} in 0..self.${field_name}.len() {
      ${vec_name}.push(self.${field_name}[${counter_name}]);
  }";

// ${type}: Struct/enum/union type.
// ${vec_name}: Name of anonymous Vec.
// ${counter_name}: Name of anonymous counter.
// ${field_name}: Field name.
// FIXME: use this for fields which are f: Vec<Type> or f: &Vec<Type>, need to use &.
/* Ex: Type {X} has field {field1} of Vec or array type, and we want to copy over the
   contents into a temporary to pass to print routines.

  let mut {__daikon_tmp1}: Vec<&{X}> = Vec::new();
  for {__daikon_tmp2} in 0..self.{field1}.len() {
      {__daikon_tmp1}.push(&self.{field1}[{__daikon_tmp2}]);
  }

*/
pub(crate) static DTRACE_TMP_VEC_FOR_FIELD_AMPERSAND: &str =
    "let mut ${vec_name}: Vec<&${type}> = Vec::new();
  for ${counter_name} in 0..self.${field_name}.len() {
      ${vec_name}.push(&self.${field_name}[${counter_name}]);
  }";

// ${field_name}: Field name.
/* Ex: print a vec field as a pointer named {field1}. Remove when Vecs are not a special case.

  dtrace_print_pointer(self.{field1}.as_ptr() as usize, format!("{}{}", prefix, ".{field1}"));

*/
pub(crate) static DTRACE_POINTER_VEC_USERDEF: &str = "dtrace_print_pointer(self.${field_name}.as_ptr() as usize, format!(\"{}{}\", prefix, \".${field_name}\"));";

// ${field_name}: Field name.
/* Ex: print {field1} as a pointer.

  dtrace_print_pointer(self.{field1}, as *const () as usize, format!("{}{}", prefix, ".{field1}"));

*/
pub(crate) static DTRACE_POINTER_ARR_USERDEF: &str = "dtrace_print_pointer(self.${field_name}, as *const () as usize, format!(\"{}{}\", prefix, \".${field_name}\"));";

// ${type}: Struct/enum/union type.
// ${vec_name}: Name of anonymous Vec.
// ${field_name}: Field name.
/* Ex: Type {X} has a field {field1} that is a Vec and we want to print the pointer values.

  dtrace_print_pointer_vec::<{X}>(&{__daikon_tmp1}, format!("{}{}[..]", prefix, ".{field1}"));

*/
pub(crate) static DTRACE_POINTERS_VEC_USERDEF: &str = "dtrace_print_pointer_vec::<${type}>(&${vec_name}, format!(\"{}{}[..]\", prefix, \".${field_name}\"));";

// ${type}: Struct/enum/union type.
// ${vec_name}: Name of anonymous Vec.
// ${field_name}: Field name.
/* Ex: Type {X} has a field {field1} that is a Vec and we want to print the values.

  {X}::dtrace_print_fields_vec(&{__daikon_tmp1}, depth - 1, format!("{}{}[..]", prefix, ".{field1}"));

*/
pub(crate) static DTRACE_PRINT_FIELDS_FOR_FIELD: &str = "${type}::dtrace_print_fields_vec(&${vec_name}, depth - 1, format!(\"{}{}[..]\", prefix, \".${field_name}\"));";

// ${prim_type}: Primitive type.
// ${vec_name}: Anonymous Vec name.
// ${counter_name}: Anonymous counter name.
// ${field_name}: Field name.
/* Ex: For a type X with field1 a Vec of primitives, read the field into a temporary
   variable to process in instrumentation.

  let mut {__daikon_tmp1}: Vec<{i32}> = Vec::new();
  for {__daikon_tmp2} in 0..self.{field1}.len() {
      {__daikon_tmp1}.push({i32}::from_str(&self.{field1}[{__daikon_tmp2}].to_string()).expect("Convert failed"));
  }

*/
pub(crate) static DTRACE_TMP_PRIM_VEC_FOR_FIELD: &str =
  "let mut ${vec_name}: Vec<${prim_type}> = Vec::new();
  for ${counter_name} in 0..self.${field_name}.len() {
      ${vec_name}.push(${prim_type}::from_str(&self.${field_name}[${counter_name}].to_string()).expect(\"Variable parsing failed\"));
  }";

// ${prim_type}: Primitive type
// ${vec_name}: Name of anonymous Vec.
// ${field_name}: Field name.
/* Ex: When we have a type X with field {field1} that has type Vec<{i32}> or some other primitive.
   Don't need this when we stop special-casing on Vec.

  dtrace_print_prim_vec::<{i32}>(&{__daikon_tmp1}, format!("{}{}", prefix, ".{field1}"));

*/
pub(crate) static DTRACE_PRINT_PRIM_VEC_FOR_FIELD: &str = "dtrace_print_prim_vec::<${prim_type}>(&${vec_name}, format!(\"{}{}\", prefix, \".${field_name}\"));";

// ${vec_name}: Name of anonymous Vec.
// ${field_name}: Field name.
/* Ex: used when a type X has a field {field1} with type Vec<string_type> and we
   have transported the contents of the field into the temporary Vec __daikon_tmp1.
   Call dtrace_print_string_vec on the temporary with the right prefix to log this more
   complex case of a primitive field (until we stop special-casing on Vec!).

  dtrace_print_string_vec(&{__daikon_tmp1}, format!("{}{}", prefix, ".{field1}"));

*/
pub(crate) static DTRACE_PRINT_STRING_VEC_FOR_FIELD: &str =
    "dtrace_print_string_vec(&${vec_name}, format!(\"{}{}\", prefix, \".${field_name}\"));";

// ${field_name}: Field name.
// Subroutine of dtrace_print_fields (I think) for when {field1} is a non-primitive type.
/* Ex:

  self.dtrace_print_{field1}(depth, prefix.clone());

*/
pub(crate) static DTRACE_CALL_PRINT_FIELD: &str =
    "self.dtrace_print_${field_name}(depth, prefix.clone());";

// ${ret_ty}: Return type.
// ${ret_expr}: Return expression.
/* Ex: Given return type {R} and expression that is returned {ret_expr}:

  fn foo() {
      let __daikon_ret: {R} = {ret_expr};
  }

*/
pub(crate) static DTRACE_LET_RET: &str = "fn foo() {
      let __daikon_ret: ${ret_ty} = ${ret_expr};
  }";

/* Ex:

  fn foo() {
      return __daikon_ret;
  }

*/
pub(crate) static DTRACE_RET: &str = "fn foo() {
      return __daikon_ret;
  }";

// ${variable_name}: Variable name.
/* Ex:

  dtrace_print_pointer({parameter1} as *const _ as *const () as usize, String::from("{parameter1}"));

*/
pub(crate) static DTRACE_BUILD_POINTER_ARR: &str = "dtrace_print_pointer(${variable_name} as *const _ as *const () as usize, String::from(\"${variable_name}\"));";

// does this work for references?
/* Ex:

  dtrace_print_pointer(__daikon_ret as *const _ as *const () as usize, String::from("return"));

*/
pub(crate) static DTRACE_BUILD_POINTER_ARR_RET: &str = "dtrace_print_pointer(__daikon_ret as *const _ as *const () as usize, String::from(\"return\"));";

// ${prim_type}: Primitive type.
// ${vec_name}: Name of anonymous Vec.
// ${variable_name}: Variable name.
// only end fn __skip because we will smash a tmp vec loop on the front.
/* Ex:

    dtrace_print_prim_vec::<{i32}>(&{__daikon_tmp1}, String::from("{parameter1}"));
  } // closes what?

*/
pub(crate) static DTRACE_PRINT_PRIM_VEC: &str =
    "dtrace_print_prim_vec::<${prim_type}>(&${vec_name}, String::from(\"${variable_name}\"));
  }";

// ${vec_name}: Name of anonymous Vec.
// ${variable_name}: Variable name.
/* Ex:

    dtrace_print_string_vec(&{__daikon_tmp1}, String::from("{parameter1}"));
  } // closes what? A synthesized function? If so, move it do dtrace_routine_builders.

*/
pub(crate) static DTRACE_PRINT_STRING_VEC: &str =
    "dtrace_print_string_vec(&${vec_name}, String::from(\"{parameter1}\"));
  }";

/* Ex.

  fn foo() { return; }

*/
pub(crate) static DTRACE_VOID_RETURN: &str = "fn foo() { return; }";

/* Ex.

  fn foo() { dtrace_newline(); }

*/
pub(crate) static DTRACE_NEWLINE: &str = "fn foo() { dtrace_newline(); }";

// this NONCE_COUNTER per-file is broken for multi-file non-concurrent programs. It has to be a single counter shared between all the files.
// Difficult in Rust as there is no easy extern escape like in C. Maybe unsafe.
pub(crate) static DTRACE_IMPORTS: &str = "use std::fs::File;
    use std::io::prelude::*;
    use std::sync::{LazyLock, Mutex};
    use std::str::FromStr;
    static NONCE_COUNTER: LazyLock<Mutex<u32>> = LazyLock::new(|| Mutex::new(0));";

/* FIXME:
 * UNUSED ROUTINES (bad, either remove unnecessary routines or add missing functionality):

  dtrace_print_pointer_arr
  dtrace_print_prim_arr
  dtrace_print_str
  dtrace_entry_no_nonce
  dtrace_exit_no_nonce

*/

pub(crate) static DAIKON_LIB: [&str; 15] = [
    "pub fn dtrace_print_pointer_arr<T>(v: &[&T], var_name: String) {
    let mut traces = match File::options().append(true).open(\"",
    ".dtrace\") {
        Err(why) => panic!(\"Daikon couldn't open file, {}\", why),
        Ok(traces) => traces,
    };
    writeln!(&mut traces, \"{}\", var_name).ok();
    let mut arr = String::from(\"[\");
    let mut i = 0;
    while i+1 < v.len() {
        arr.push_str(&format!(\"0x{:x} \", v[i] as *const _ as usize));
        i += 1;
    }
    if v.len() > 0 {
        arr.push_str(&format!(\"0x{:x}\", v[i] as *const _ as usize));
    }
    arr.push_str(\"]\");
    writeln!(&mut traces, \"{}\", arr).ok();
    writeln!(&mut traces, \"0\").ok();
}

pub fn dtrace_print_pointer_vec<T>(v: &Vec<&T>, var_name: String) {
    let mut traces = match File::options().append(true).open(\"",
    ".dtrace\") {
        Err(why) => panic!(\"Daikon couldn't open file, {}\", why),
        Ok(traces) => traces,
    };
    writeln!(&mut traces, \"{}\", var_name).ok();
    let mut arr = String::from(\"[\");
    let mut i = 0;
    while i+1 < v.len() {
        arr.push_str(&format!(\"0x{:x} \", v[i] as *const _ as usize));
        i += 1;
    }
    if v.len() > 0 {
        arr.push_str(&format!(\"0x{:x}\", v[i] as *const _ as usize));
    }
    arr.push_str(\"]\");
    writeln!(&mut traces, \"{}\", arr).ok();
    writeln!(&mut traces, \"0\").ok();
}

// T must implement Display trait
fn dtrace_print_prim_arr<T: std::fmt::Display>(v: &[T], prefix: String) {
    let mut traces = match File::options().append(true).open(\"",
    ".dtrace\") {
        Err(why) => panic!(\"Daikon couldn't open file, {}\", why),
        Ok(traces) => traces,
    };
    writeln!(&mut traces, \"{}\", format!(\"{}{}\", prefix, \"[..]\")).ok();
    let mut arr = String::from(\"[\");
    let mut i = 0;
    while i+1 < v.len() {
        arr.push_str(&format!(\"{} \", v[i]));
        i += 1;
    }
    if v.len() > 0 {
        arr.push_str(&format!(\"{}\", v[i]));
    }
    arr.push_str(\"]\");
    writeln!(&mut traces, \"{}\", arr).ok();
    writeln!(&mut traces, \"0\").ok();
}

fn dtrace_print_prim_vec<T: std::fmt::Display>(v: &Vec<T>, prefix: String) {
    let mut traces = match File::options().append(true).open(\"",
    ".dtrace\") {
        Err(why) => panic!(\"Daikon couldn't open file, {}\", why),
        Ok(traces) => traces,
    };
    writeln!(&mut traces, \"{}\", format!(\"{}{}\", prefix, \"[..]\")).ok();
    let mut arr = String::from(\"[\");
    let mut i = 0;
    while i+1 < v.len() {
        arr.push_str(&format!(\"{} \", v[i]));
        i += 1;
    }
    if v.len() > 0 {
        arr.push_str(&format!(\"{}\", v[i]));
    }
    arr.push_str(\"]\");
    writeln!(&mut traces, \"{}\", arr).ok();
    writeln!(&mut traces, \"0\").ok();
}

fn dtrace_print_str(v: &str, var_name: String) {
    let mut traces = match File::options().append(true).open(\"",
    ".dtrace\") {
        Err(why) => panic!(\"Daikon couldn't open file, {}\", why),
        Ok(traces) => traces,
    };
    writeln!(&mut traces, \"{}\", var_name).ok();
    writeln!(&mut traces, \"{}\", v).ok();
    writeln!(&mut traces, \"0\").ok();
}

// T must implement Display trait
fn dtrace_print_prim<T: std::fmt::Display>(v: T, var_name: String) {
    let mut traces = match File::options().append(true).open(\"",
    ".dtrace\") {
        Err(why) => panic!(\"Daikon couldn't open file, {}\", why),
        Ok(traces) => traces,
    };
    writeln!(&mut traces, \"{}\", var_name).ok();
    writeln!(&mut traces, \"{}\", v).ok();
    writeln!(&mut traces, \"0\").ok();
}

fn dtrace_print_string(v: String, var_name: String) {
    let mut traces = match File::options().append(true).open(\"",
    ".dtrace\") {
        Err(why) => panic!(\"Daikon couldn't open file, {}\", why),
        Ok(traces) => traces,
    };
    writeln!(&mut traces, \"{}\", var_name).ok();
    writeln!(&mut traces, \"\\\"{}\\\"\", v).ok();
    writeln!(&mut traces, \"0\").ok();
}

fn dtrace_print_string_vec(v: &Vec<String>, prefix: String) {
    let mut traces = match File::options().append(true).open(\"",
    ".dtrace\") {
        Err(why) => panic!(\"Daikon couldn't open file, {}\", why),
        Ok(traces) => traces,
    };
    writeln!(&mut traces, \"{}\", format!(\"{}{}\", prefix, \"[..]\")).ok();
    let mut arr = String::from(\"[\");
    let mut i = 0;
    while i+1 < v.len() {
        arr.push_str(&format!(\"\\\"{}\\\" \", v[i]));
        i += 1;
    }
    if v.len() > 0 {
        arr.push_str(&format!(\"\\\"{}\\\"\", v[i]));
    }
    arr.push_str(\"]\");
    writeln!(&mut traces, \"{}\", arr).ok();
    writeln!(&mut traces, \"0\").ok();
}

fn dtrace_print_pointer(v: usize, var_name: String) {
    let mut traces = match File::options().append(true).open(\"",
    ".dtrace\") {
        Err(why) => panic!(\"Daikon couldn't open file, {}\", why),
        Ok(traces) => traces,
    };
    writeln!(&mut traces, \"{}\", var_name).ok();
    writeln!(&mut traces, \"0x{:x}\", v).ok();
    writeln!(&mut traces, \"0\").ok();
}

fn dtrace_entry_no_nonce(ppt_name: &str) {
    let mut traces = match File::options().append(true).open(\"",
    ".dtrace\") {
        Err(why) => panic!(\"Daikon couldn't open file, {}\", why),
        Ok(traces) => traces,
    };
    writeln!(&mut traces, \"{}\", ppt_name).ok();
}

fn dtrace_exit_no_nonce(ppt_name: &str) {
    let mut traces = match File::options().append(true).open(\"",
    ".dtrace\") {
        Err(why) => panic!(\"Daikon couldn't open file, {}\", why),
        Ok(traces) => traces,
    };
    writeln!(&mut traces, \"{}\", ppt_name).ok();
}

fn dtrace_entry(ppt_name: &str, nonce: u32) {
    let mut traces = match File::options().append(true).open(\"",
    ".dtrace\") {
        Err(why) => panic!(\"Daikon couldn't open file, {}\", why),
        Ok(traces) => traces,
    };
    writeln!(&mut traces, \"{}\", ppt_name).ok();
    writeln!(&mut traces, \"this_invocation_nonce\").ok();
    writeln!(&mut traces, \"{}\", nonce).ok();
}

fn dtrace_exit(ppt_name: &str, nonce: u32) {
    let mut traces = match File::options().append(true).open(\"",
    ".dtrace\") {
        Err(why) => panic!(\"Daikon couldn't open file, {}\", why),
        Ok(traces) => traces,
    };
    writeln!(traces, \"{}\", ppt_name).ok();
    writeln!(traces, \"this_invocation_nonce\").ok();
    writeln!(traces, \"{}\", nonce).ok();
}

fn dtrace_newline() {
    let mut traces = match File::options().append(true).open(\"",
    ".dtrace\") {
        Err(why) => panic!(\"Daikon couldn't open file, {}\", why),
        Ok(traces) => traces,
    };
    writeln!(traces, \"\").ok();
}",
];

pub(crate) fn daikon_lib() -> String {
    let mut res = String::from(DAIKON_LIB[0]);
    for i in 1..DAIKON_LIB.len() {
        res.push_str(&*OUTPUT_PREFIX.lock().unwrap());
        res.push_str(DAIKON_LIB[i]);
    }
    res
}

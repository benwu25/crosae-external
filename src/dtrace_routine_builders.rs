/* Ex: Build a subroutine of dtrace_print_fields. Called like
   self.dtrace_print_{field1}(depth - 1, some_prefix);

  pub fn dtrace_print_{field1}(&self, depth: i32, prefix: String) {

      ... (emit primitive fields at this level)

      if depth == 0 { return; }

      ... (potentially recurse)

  }

*/
// ${field_name}: Field name.
pub(crate) static DTRACE_PRINT_XFIELD_FOR_FIELD_PROLOGUE: &str =
    "pub fn dtrace_print_${field_name}(&self, depth: i32, prefix: String) {";
pub(crate) static DTRACE_PRINT_XFIELD_FOR_FIELD_MID: &str = "if depth == 0 { return; }";
pub(crate) static DTRACE_PRINT_XFIELD_FOR_FIELD_EPILOGUE: &str = "}";

// ${type}: Struct/union/enum type.
// ${field_name}: Field name.
// ${file_name}: Program name.
// NOTE: not sure why this is unused. as_ptr() is for Vecs, so this should be used
// when {field1} is a Vec. That is, until we stop special-casing for Vecs!
/* Ex: (note that {:x} is not being replaced, just a format specifier for hex.

  pub fn dtrace_print_{field1}_vec(v: &Vec<&{X}>, var_name: String) {
      let mut traces = match File::options().append(true).open("{file_name}.dtrace") {
          Err(why) => panic!("Daikon couldn't open file, {}", why),
          Ok(traces) => traces,
      };
      writeln!(&mut traces, "{}", var_name).ok();
      let mut arr = String::from("[");
      for i in 0..v.len()-1 {
          arr.push_str(&format!("0x{:x}", v[i].{field1}.as_ptr() as usize));
      }
      if v.len() > 0 {
          arr.push_str(&format!("0x{:x}", v[v.len() - 1].{field1}.as_ptr() as usize));
      }
      arr.push_str("]");
      writeln!(&mut traces, "{}", arr).ok();
      writeln!(traces, "0").ok();
  }

*/
pub(crate) static DTRACE_PRINT_XFIELDS_VEC: &str =
    "pub fn dtrace_print_${field_name}_vec(v: &Vec<&${type}>, var_name: String) {
      let mut traces = match File::options().append(true).open(\"${file_name}.dtrace\") {
          Err(why) => panic!(\"Daikon couldn't open file, {}\", why),
          Ok(traces) => traces,
      };
      writeln!(&mut traces, \"{}\", var_name).ok();
      let mut arr = String::from(\"[\");
      for i in 0..v.len() - 1 {
          arr.push_str(&format!(\"0x{:x}\", v[i].${field_name}.as_ptr() as usize));
      }
      if v.len() > 0 {
          arr.push_str(&format!(\"0x{:x}\", v[v.len() - 1].${field_name}.as_ptr() as usize));
      }
      arr.push_str(\"]\");
      writeln!(&mut traces, \"{}\", arr).ok();
      writeln!(traces, \"0\").ok();
  }";

/* Ex.

  impl foo {
      pub fn dtrace_print_fields(&self, depth: i32, prefix: String) {
          if depth == 0 { return; }

*/
// you have to delete this?
// make this an array with DTRACE_PRINT_FIELDS_EPILOGUE...
// Note: the start of standard dtrace_print_fields for some type.
pub(crate) static DTRACE_PRINT_FIELDS_PROLOGUE: &str = "impl foo {
        pub fn dtrace_print_fields(&self, depth: i32, prefix: String) { if depth == 0 { return; } ";

// Note, I think the trailing "struct foo{}" is there to avoid basic type-checking errors when
// parsing the string fragment, since we can't define impl foo without a struct foo
// definition, thought parsing shouldn't care.
/* Ex.

          }
      }
      struct foo { }

*/
pub(crate) static DTRACE_PRINT_FIELDS_EPILOGUE: &str = "}
      }
      struct foo { }";

// ${type}: Struct/enum/union type.
/* Ex. The beginning of dtrace_print_fields_vec which is defined in a new impl block for the type
   {X}.

  impl foo {
      pub fn dtrace_print_fields_vec(v: &Vec<&{X}>, depth: i32, prefix: String) {
          if depth == 0 { return; }

*/
pub(crate) static DTRACE_PRINT_FIELDS_VEC_PROLOGUE: &str = "impl foo {
      pub fn dtrace_print_fields_vec(v: &Vec<&${type}>, depth: i32, prefix: String) {
          if depth == 0 { return; }";

// Note, this completes the impl block and fn started above.
/* Ex.

          }
      }
      struct foo { }

*/
pub(crate) static DTRACE_PRINT_FIELDS_VEC_EPILOGUE: &str = "}
      }
      struct foo { }";

/* Ex.

  impl foo {

*/
pub(crate) static DTRACE_PRINT_XFIELDS_VEC_PROLOGUE: &str = "impl foo {";

/* Ex.

  }

*/
pub(crate) static DTRACE_PRINT_XFIELDS_VEC_EPILOGUE: &str = " }";

// ${type}: Struct/enum/union type.
// ${field_name}: Field name.
// ${file_name}: File name.
/* Ex. See below. Identical, except no extra quotations are needed.

  pub fn dtrace_print_{field1}_vec(v: &Vec<&{X}>, var_name: String) {
      let mut traces = match File::options().append(true).open("{program_name}.dtrace") {
          Err(why) => panic!("Daikon couldn't open file, {}", why),
          Ok(traces) => traces,
      };
      writeln!(&mut traces, "{}", var_name).ok();
      let mut arr = String::from("[");
      for i in 0..v.len() - 1 { // NOTE: a check for length would be wise to insert here.
          arr.push_str(&format!("{}", v[i].{field1}));
      }
      if v.len() > 0 {
          arr.push_str(&format!("{}", v[v.len() - 1].{field1}));
      }
      arr.push_str("]");
      writeln!(&mut traces, "{}", arr).ok();

      writeln!(traces, "0").ok(); // why no &mut traces?
  }
*/
pub(crate) static DTRACE_PRINT_XFIELDS: &str =
    "pub fn dtrace_print_${field_name}_vec(v: &Vec<&${type}>, var_name: String) {
      let mut traces = match File::options().append(true).open(\"{file_name}.dtrace\") {
          Err(why) => panic!(\"Daikon couldn't open file, {}\", why),
          Ok(traces) => traces,
      };
      writeln!(&mut traces, \"{}\", var_name).ok();
      let mut arr = String::from(\"[\");
      for i in 0..v.len() - 1 { // NOTE: a check for length would be wise to insert here.
          arr.push_str(&format!(\"{}\", v[i].${field_name}));
      }
      if v.len() > 0 {
          arr.push_str(&format!(\"{}\", v[v.len() - 1].${field_name}));
      }
      arr.push_str(\"]\");
      writeln!(&mut traces, \"{}\", arr).ok();

      writeln!(traces, \"0\").ok(); // why no &mut traces?
  }";

// ${type}: Struct/enum/union type.
// ${field_name}: Field name.
// ${file_name}: File name.
/* Ex. field1 is a field belonging to the type X, and field1 has a string type.
   I.e., it has type String or &str, so we insert extra quotes in the format so
   Daikon knows it is a String and doesn't complain.

  pub fn dtrace_print_{field1}_vec(v: &Vec<&{X}>, var_name: String) {
      let mut traces = match File::options().append(true).open("{program_name}.dtrace") {
          Err(why) => panic!("Daikon couldn't open file, {}", why),
          Ok(traces) => traces,
      };
      writeln!(&mut traces, "{}", var_name).ok();
      let mut arr = String::from("[");
      for i in 0..v.len() - 1 {
          arr.push_str(&format!(" \"{}\" ", v[i].{field1}));
      }
      if v.len() > 0 {
          arr.push_str(&format!(" \"{}\" ", v[v.len() - 1].{field1}));
      }
      arr.push_str("]");
      writeln!(&mut traces, "{}", arr).ok();

      writeln!(traces, "0").ok(); // why no &mut traces?
  }

*/
pub(crate) static DTRACE_PRINT_XFIELDS_STRING: &str =
    "pub fn dtrace_print_${field_name}_vec(v: &Vec<&${type}>, var_name: String) {
      let mut traces = match File::options().append(true).open(\"${file_name}.dtrace\") {
          Err(why) => panic!(\"Daikon couldn't open file, {}\", why),
          Ok(traces) => traces,
      };
      writeln!(&mut traces, \"{}\", var_name).ok();
      let mut arr = String::from(\"[\");
      for i in 0..v.len() - 1 {
          arr.push_str(&format!(\" \\ \"{}\\ \" \", v[i].${field_name}));
      }
      if v.len() > 0 {
          arr.push_str(&format!(\" \\ \"{}\" \\ \", v[v.len() - 1].${field_name}));
      }
      arr.push_str(\"]\");
      writeln!(&mut traces, \"{}\", arr).ok();

      writeln!(traces, \"0\").ok(); // why no &mut traces?
  }";

/* Ex.

  impl foo() {}

*/
pub(crate) static DTRACE_BUILD_AN_IMPL_BLOCK: &str = "impl foo() {}";

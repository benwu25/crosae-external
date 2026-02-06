use std::{collections::HashMap, path::Path};

use crate::common::{compile_and_execute, verify};

#[test]
fn collections() {
    let mut expected = HashMap::new();
    expected.insert("main::ENTER", HashMap::new());
    expected.insert("main::EXIT", HashMap::new());
    expected.insert("foo::ENTER", HashMap::from([("x", 0), ("y", 1)]));
    expected.insert("foo::EXIT", HashMap::from([("x", 0), ("y", 0), ("RET", 1)]));
    expected.insert("bar::ENTER", HashMap::from([("a", 0), ("b", 2)]));
    expected.insert("bar::EXIT", HashMap::from([("a", 0), ("b", 0), ("RET", 0)]));
    expected.insert("baz::ENTER", HashMap::from([("a", 0), ("b", 1)]));
    expected.insert("baz::EXIT", HashMap::from([("a", 0), ("b", 0)]));

    let test_dir = Path::new(file!()).parent().unwrap().to_str().unwrap();
    let ati_output = compile_and_execute(test_dir, "collections");
    verify(&ati_output, &expected);
}

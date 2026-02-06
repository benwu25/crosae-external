use std::{collections::HashMap, path::Path};

use crate::common::{compile_and_execute, verify};

#[test]
fn different_kinds_of_returns() {
    let mut expected = HashMap::new();
    expected.insert("main::ENTER", HashMap::new());
    expected.insert("main::EXIT", HashMap::new());
    expected.insert(
        "implicit_return::ENTER",
        HashMap::from([("x", 0), ("y", 1), ("z", 2)]),
    );
    expected.insert(
        "implicit_return::EXIT",
        HashMap::from([("x", 0), ("y", 0), ("z", 1), ("RET", 0)]),
    );
    expected.insert(
        "explicit_return::ENTER",
        HashMap::from([("x", 0), ("y", 1), ("z", 2)]),
    );
    expected.insert(
        "explicit_return::EXIT",
        HashMap::from([("x", 1), ("y", 0), ("z", 0), ("RET", 0)]),
    );
    expected.insert(
        "explicit_unsemi_return::ENTER",
        HashMap::from([("x", 0), ("y", 1), ("z", 2)]),
    );
    expected.insert(
        "explicit_unsemi_return::EXIT",
        HashMap::from([("x", 0), ("y", 1), ("z", 0), ("RET", 0)]),
    );
    expected.insert(
        "nested_implicit_return::ENTER",
        HashMap::from([("x", 0), ("y", 0), ("z", 1)]),
    );
    expected.insert(
        "nested_implicit_return::EXIT",
        HashMap::from([("x", 0), ("y", 0), ("z", 0), ("RET", 0)]),
    );
    expected.insert(
        "nested_explicit_return::ENTER",
        HashMap::from([("x", 0), ("y", 0), ("z", 1)]),
    );
    expected.insert(
        "nested_explicit_return::EXIT",
        HashMap::from([("x", 0), ("y", 0), ("z", 0), ("RET", 0)]),
    );

    let test_dir = Path::new(file!()).parent().unwrap().to_str().unwrap();
    let ati_output = compile_and_execute(test_dir, "returns");
    verify(&ati_output, &expected);
}

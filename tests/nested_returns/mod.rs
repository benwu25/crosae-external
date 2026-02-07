use std::{collections::HashMap, path::Path};

use crate::common::{compile_and_execute, delete, verify};

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

    let executable = Path::new(file!()).parent().unwrap().join("returns.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, &expected);
}

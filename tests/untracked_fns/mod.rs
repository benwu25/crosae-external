use std::{collections::HashMap, path::Path};

use crate::common::{compile_and_execute, delete, verify};

#[test]
fn untracked_fns() {
    let mut expected = HashMap::new();
    expected.insert("main::ENTER", HashMap::new());
    expected.insert("main::EXIT", HashMap::new());
    expected.insert("foo::ENTER", HashMap::from([
        ("a", 0),
        ("b", 1),
        ("c", 2),
        ("d", 3),
        ("e", 4),
    ]));
    expected.insert("foo::EXIT", HashMap::from([
        ("a", 0),
        ("b", 1),
        ("c", 2),
        ("d", 2),
        ("e", 3),
        ("RET", 3)
    ]));
    expected.insert("max::ENTER", HashMap::from([
        ("a", 0),
        ("b", 1),
    ]));
    expected.insert("max::EXIT", HashMap::from([
        ("a", 0),
        ("b", 0),
        ("RET", 0)
    ]));

    let executable = Path::new(file!()).parent().unwrap().join("untracked_fns.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, &expected);
}

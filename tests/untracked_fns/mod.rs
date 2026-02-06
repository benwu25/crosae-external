use std::{collections::HashMap, path::Path};

use crate::common::{compile_and_execute, verify};

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

    let test_dir = Path::new(file!()).parent().unwrap().to_str().unwrap();
    let ati_output = compile_and_execute(test_dir, "untracked_fns");
    verify(&ati_output, &expected);
}

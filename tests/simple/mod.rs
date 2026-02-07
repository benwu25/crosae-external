use std::{collections::HashMap, path::Path};

use crate::common::{compile_and_execute, verify, delete};

#[test]
fn simple() {
    let mut expected = HashMap::new();
    expected.insert("main::ENTER", HashMap::new());
    expected.insert("main::EXIT", HashMap::new());
    expected.insert("foo::ENTER", HashMap::from([("x", 0), ("y", 1), ("z", 2)]));
    expected.insert(
        "foo::EXIT",
        HashMap::from([("x", 0), ("y", 0), ("z", 1), ("RET", 0)]),
    );

    let executable = Path::new(file!()).parent().unwrap().join("simple.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, &expected);
}

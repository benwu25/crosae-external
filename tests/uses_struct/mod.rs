use std::{collections::HashMap, path::Path};

use crate::common::{compile_and_execute, delete, verify};

#[test]
fn uses_struct() {
    let mut expected = HashMap::new();
    expected.insert("main::ENTER", HashMap::new());
    expected.insert("main::EXIT", HashMap::new());
    expected.insert("func::ENTER", HashMap::from([("x", 0), ("y", 1), ("z", 2)]));
    expected.insert(
        "func::EXIT",
        HashMap::from([("x", 0), ("y", 0), ("z", 1), ("RET", 0)]),
    );

    let executable = Path::new(file!()).parent().unwrap().join("struct.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, &expected);
}

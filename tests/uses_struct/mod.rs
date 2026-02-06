use std::{collections::HashMap, path::Path};

use crate::common::{compile_and_execute, verify};

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

    let test_dir = Path::new(file!()).parent().unwrap().to_str().unwrap();
    let ati_output = compile_and_execute(test_dir, "struct");
    verify(&ati_output, &expected);
}

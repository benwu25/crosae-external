use std::{collections::HashMap, path::Path};

use crate::common::{compile_and_execute, verify};

// TODO: probably a good idea to make sites qualify the file name they are in too
#[test]
fn multi_file() {
    let mut expected = HashMap::new();
    expected.insert("main::ENTER", HashMap::new());
    expected.insert("main::EXIT", HashMap::new());
    expected.insert("foo::ENTER", HashMap::from([("x", 0), ("y", 1), ("z", 2)]));
    expected.insert("foo::EXIT", HashMap::from([("x", 0), ("y", 0), ("z", 1), ("RET", 0)]));
    expected.insert("from_dep::ENTER", HashMap::from([("x", 0), ("y", 1), ("z", 2)]));
    expected.insert("from_dep::EXIT", HashMap::from([("x", 0), ("y", 1), ("z", 1), ("RET", 1)]));

    let test_dir = Path::new(file!()).parent().unwrap().to_str().unwrap();
    let ati_output = compile_and_execute(test_dir, "multi_file");
    verify(&ati_output, &expected);
}

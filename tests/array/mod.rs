use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

#[test]
fn array() {
    let mut expected = ExpectedOutput::new();

    expected.register_site(ExpectedSite::new("main::ENTER"));
    expected.register_site(ExpectedSite::new("main::EXIT"));

    expected.register_site(
        ExpectedSite::new("foo::ENTER")
            .register("x", 0)
            .register("y", 1)
            .register("z", 2)
            .register_array("arr", 3, 3),
    );
    expected.register_site(
        ExpectedSite::new("foo::EXIT")
            .register_array("arr", 3, 0)
            .register("x", 0)
            .register("y", 0)
            .register("z", 1)
            .register("RET", 0),
    );

    let executable = Path::new(file!()).parent().unwrap().join("array.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}

use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

#[test]
fn uses_struct() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new("main::ENTER"));
    expected.register_site(ExpectedSite::new("main::EXIT"));
    expected.register_site(
        ExpectedSite::new("func::ENTER")
            .register("x", 0)
            .register("y", 1)
            .register("z", 2)
            .register("s.x", 3)
            .register("s.y", 4)
            .register("s.z.x", 5)
    );
    expected.register_site(
        ExpectedSite::new("func::EXIT")
            .register("x", 0)
            .register("y", 0)
            .register("z", 1)
            .register("s.x", 0)
            .register("s.y", 0)
            .register("s.z.x", 2)
            .register("RET", 0),
    );

    let executable = Path::new(file!()).parent().unwrap().join("struct.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}

use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

#[test]
fn collections() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new("main::ENTER"));
    expected.register_site(ExpectedSite::new("main::EXIT"));

    expected.register_site(
        ExpectedSite::new("foo::ENTER")
            .register("x", 0)
            .register("y", 1),
    );
    expected.register_site(
        ExpectedSite::new("foo::EXIT")
            .register("x", 0)
            .register("y", 0)
            .register("RET", 1),
    );
    expected.register_site(
        ExpectedSite::new("bar::ENTER")
            .register("a", 0)
            .register("b", 2),
    );
    expected.register_site(
        ExpectedSite::new("bar::EXIT")
            .register("a", 0)
            .register("b", 0)
            .register("RET", 0),
    );
    expected.register_site(
        ExpectedSite::new("baz::ENTER")
            .register("a", 0)
            .register("b", 1),
    );
    expected.register_site(
        ExpectedSite::new("baz::EXIT")
            .register("a", 0)
            .register("b", 0),
    );

    let executable = Path::new(file!()).parent().unwrap().join("collections.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}

// TODO:
// 1. Delete files at start of each test
// 2. Fix unit tests not always running.

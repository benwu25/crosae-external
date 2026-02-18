use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

#[test]
fn different_kinds_of_returns() {
    let mut output = ExpectedOutput::new();
    output.register_site(ExpectedSite::new("main::ENTER"));
    output.register_site(ExpectedSite::new("main::EXIT"));

    output.register_site(
        ExpectedSite::new("implicit_return::ENTER")
            .register("x", 0)
            .register("y", 1)
            .register("z", 2),
    );
    output.register_site(
        ExpectedSite::new("implicit_return::EXIT")
            .register("x", 0)
            .register("y", 0)
            .register("z", 1)
            .register("RET", 0),
    );
    output.register_site(
        ExpectedSite::new("explicit_return::ENTER")
            .register("x", 0)
            .register("y", 1)
            .register("z", 2),
    );
    output.register_site(
        ExpectedSite::new("explicit_return::EXIT")
            .register("x", 1)
            .register("y", 0)
            .register("z", 0)
            .register("RET", 0),
    );
    output.register_site(
        ExpectedSite::new("explicit_unsemi_return::ENTER")
            .register("x", 0)
            .register("y", 1)
            .register("z", 2),
    );
    output.register_site(
        ExpectedSite::new("explicit_unsemi_return::EXIT")
            .register("x", 0)
            .register("y", 1)
            .register("z", 0)
            .register("RET", 0),
    );

    output.register_site(
        ExpectedSite::new("nested_implicit_return::ENTER")
            .register("x", 0)
            .register("y", 0)
            .register("z", 1),
    );

    output.register_site(
        ExpectedSite::new("nested_implicit_return::EXIT")
            .register("x", 0)
            .register("y", 0)
            .register("z", 0)
            .register("RET", 0),
    );

    output.register_site(
        ExpectedSite::new("nested_explicit_return::ENTER")
            .register("x", 0)
            .register("y", 0)
            .register("z", 1),
    );

    output.register_site(
        ExpectedSite::new("nested_explicit_return::EXIT")
            .register("x", 0)
            .register("y", 0)
            .register("z", 0)
            .register("RET", 0),
    );

    let executable = Path::new(file!()).parent().unwrap().join("returns.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, output.inner());
}

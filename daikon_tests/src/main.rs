use colored::Colorize;

// Given a Rust source file name, splice out the name of the file
// by excluding the .rs extension and any preceding directory names.
pub fn get_output_name(s: String) -> String {
    let end = match s.rfind(".") {
        // .rs
        None => panic!("no . at the end of input file name"),
        Some(end) => end,
    };
    let mut start = match s.rfind("/") {
        // .../<crate>.rs
        None => 0,
        Some(slash) => slash + 1,
    };
    let mut res = String::from("");
    while start < end {
        res.push_str(&format!("{}", s.chars().nth(start).unwrap()));
        start += 1;
    }
    return res;
}

// iterate through "./tests" and run target/debug/crosae-external for files, RUSTC=target/debug/crosae-external cargo +nightly run for multi-file tests in subdirectories.
fn run_daikon_rustc_pp_tests() {
    let in_ci = std::env::var("CROSAE_CI").is_ok();
    let test_path_str = if in_ci { "/checkout/obj/daikon_tests/test/" } else { "./test/" };
    let crosae_path= if in_ci {
        // change: ci path
        "/checkout/obj/build/host/stage2/bin/rustc"
    } else {
        "../../target/debug/crosae-external"
    };

    let test_path = std::fs::canonicalize(std::path::Path::new(&test_path_str)).unwrap();

    for entry in std::fs::read_dir(test_path.clone()).unwrap() {
        let entry = entry.unwrap();
        let path = std::fs::canonicalize(entry.path()).unwrap();
        if path.is_dir() {
            // set current_dir to canonicalize(<dir>) in Command and do cargo build with crosae.
            todo!();
        } else {
            // set current_dir to canonicalize(test_path.clone()) and execute crosae.

            let path_str = path.to_str().unwrap();
            if !path_str.ends_with("rs") {
                continue;
            } else {
                let output_name = get_output_name(String::from(path_str));
                println!("Running test {}", output_name);

                // add this: run clippy::implicit_return lint (make sure dtrace code handles
                // "return <expr>" as well as "return <expr>;"

                std::process::Command::new(&crosae_path)
                    .arg(path_str)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .current_dir(&test_path)
                    .status()
                    .expect("failed to execute c-rosae");

                // read expected/actual pp to String
                let pp_path = format!("{test_path_str}{}{}", output_name, ".pp");
                let pp_as_path = std::path::Path::new(&pp_path);
                let pp_as_path_buf = std::fs::canonicalize(pp_as_path).unwrap();
                let actual = std::fs::read_to_string(&pp_as_path_buf).unwrap();
                let pp_expected_path = format!("{test_path_str}{}-expected{}", output_name, ".pp");
                let pp_expected_as_path = std::path::Path::new(&pp_expected_path);
                let pp_expected_as_path_buf = std::fs::canonicalize(pp_expected_as_path).unwrap();
                let expected = std::fs::read_to_string(&pp_expected_as_path_buf).unwrap();

                // run the instrumented code
                // FIXME: run with multiple inputs on non-deterministic programs.
                let instrumented_program = format!("{test_path_str}{}", output_name);
                std::process::Command::new(&instrumented_program)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .expect("failed to run instrumented program");

                // FIXME: document perl dependency.
                // run dtrace-diff.pl
                let decls_path = format!("{test_path_str}{}.decls", output_name);
                let dtrace_path = format!("{test_path_str}{}.dtrace", output_name);
                let dtrace_expected_path =
                    format!("{test_path_str}{}-expected.dtrace", output_name);
                let dtrace_diff = std::process::Command::new("perl")
                    .arg(&decls_path)
                    .arg(dtrace_expected_path)
                    .arg(&dtrace_path)
                    .output()
                    .expect("failed to run dtrace-diff")
                    .stdout;

                // remove junk
                std::fs::remove_file(std::path::Path::new(&instrumented_program)).unwrap();
                std::fs::remove_file(std::path::Path::new(&decls_path)).unwrap();
                std::fs::remove_file(std::path::Path::new(&dtrace_path)).unwrap();
                let pp_path = format!("{test_path_str}{}.pp", output_name);
                std::fs::remove_file(std::path::Path::new(&pp_path)).unwrap();

                // check pretty-print diff
                assert_eq!(expected, actual);

                // check dtrace diff
                assert_eq!("", String::from_utf8_lossy(&dtrace_diff));

                println!("{}", "Pass".green());
            }
        }
    }

    println!("\n{}", "All tests passed".green());
}

fn main() {
    run_daikon_rustc_pp_tests();
}

use std::{
    collections::{HashMap, HashSet},
    process::Command,
};

/// Delimiter printed at the end of execution, denoting the start of the
/// rest of the ATI information.
const ANALYSIS_START: &'static str = "===ATI-ANALYSIS-START===\n";

/// Delimiter used in ATI information between different sites
const SITE_DELIM: &'static str = "---\n";

/// Compiles `{cwd}/{test_dir}/{file_name}.rs` with the added instrumentation
/// runs it, and returns the section of the stdout stream which contains the ATI info.
pub fn compile_and_execute(test_dir: &str, file_name: &str) -> String {
    let invocation_dir = std::env::current_dir().unwrap();
    let full_test_dir = invocation_dir.join(test_dir);
    let in_path = full_test_dir.join(format!("main.rs"));
    let out_path = full_test_dir.join(format!("{file_name}.out"));

    let in_file = in_path.to_str().unwrap();
    let out_file = out_path.to_str().unwrap();

    // Compile command
    let compile_output = Command::new("cargo")
        .args(["run", "--", in_file, "-o", out_file])
        .output()
        .unwrap();

    if !compile_output.status.success() {
        let e = String::from_utf8(compile_output.stderr).unwrap();
        panic!("Unable to compile!!! STDERR: {e}");
    }

    // Execute command
    let analysis_output = Command::new(out_file).output().unwrap();
    let exec_output = String::from(str::from_utf8(&analysis_output.stdout).unwrap());

    // chop off all print statements that have nothing to do with ATI
    let start = exec_output.find(ANALYSIS_START).unwrap();
    return exec_output[(start + ANALYSIS_START.len())..].into();
}

/// Checks that the ati stdout stream contains all the expected information,
/// performing a partition comparison, alongside making sure the right number
/// of sites were discovered.
pub fn verify(mut ati_stdout: &str, expected_partition: &HashMap<&str, HashMap<&str, usize>>) {
    // checks mappings at each site are identical to expected partition
    let mut found_sites = HashSet::new();
    while let Some(end) = ati_stdout.find(SITE_DELIM) {
        let site_info: Vec<_> = ati_stdout[..end].split("\n").collect();
        let site_name = site_info[0];

        assert!(!found_sites.contains(site_name));
        found_sites.insert(site_name);

        // map of var -> id assigned to abstract_type, at this site.
        let mut site_ati_output = HashMap::new();
        for var_info in &site_info[1..] {
            if var_info.len() < 3 {
                continue;
            }

            let var_split: Vec<_> = var_info.split(":").collect();
            site_ati_output.insert(var_split[0], str::parse::<usize>(var_split[1]).unwrap());
        }

        // site with name has to exist
        let expected_site = expected_partition.get(site_name);
        assert!(
            expected_site.is_some(),
            "Did not expect {site_name} to exist."
        );

        let expected_site = expected_site.unwrap();
        assert!(expected_site.len() == site_ati_output.len());

        // to detect differences from expected
        let mut actual_to_expected = HashMap::new();

        // go through output...
        for (var, actual_id) in site_ati_output.iter() {
            if let Some(prev_expected_id) = actual_to_expected.get(actual_id) {
                // ... this is not the first time we've seen this actual_id
                // it used to map to prev_expected_id, therefore, that should still be the case
                // with this new variable
                let expected_id = expected_site.get(var).unwrap();
                assert!(
                    *prev_expected_id == expected_id,
                    "In {site_name}, expected {var} to be in set {prev_expected_id}, but found {expected_id}"
                );
            } else {
                // first time seeing this, check that this variable does exist in the
                // expected parition
                let expected_id = expected_site.get(var);
                assert!(expected_id.is_some());

                // forever associate whatever id the analysis spit out
                // with the one we expect.
                let expected_id = expected_id.unwrap();
                actual_to_expected.insert(actual_id, expected_id);
            }
        }

        ati_stdout = &ati_stdout[(end + SITE_DELIM.len())..];
    }

    // found_sites contains no duplicates, so as long as we were able
    // to match all found_sites to those in expected_partition,
    // we have equality!
    assert!(found_sites.len() == expected_partition.len());
}

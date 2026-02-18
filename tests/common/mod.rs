use std::{
    collections::{HashMap, HashSet},
    path::Path,
    process::Command,
};

/// Delimiter printed at the end of execution, denoting the start of the
/// rest of the ATI information.
const ANALYSIS_START: &'static str = "===ATI-ANALYSIS-START===\n";

/// Delimiter used in ATI information between different sites
const SITE_DELIM: &'static str = "---\n";

/// Compiles `{cwd}/{test_dir}/{file_name}.rs` with the added instrumentation
/// runs it, and returns the section of the stdout stream which contains the ATI info.
pub fn compile_and_execute(path: &Path) -> String {
    let invocation_dir = std::env::current_dir().unwrap();
    let full_executable = invocation_dir.join(path);
    let source = full_executable.parent().unwrap().join("main.rs");

    // Compile command
    let compile_output = Command::new("cargo")
        .args([
            "run",
            "--",
            source.to_str().unwrap(),
            "-o",
            full_executable.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    if !compile_output.status.success() {
        let e = String::from_utf8(compile_output.stderr).unwrap();
        panic!("Unable to compile {path:?}. Error output:\n{e}");
    }

    // Execute command
    let analysis_output = Command::new(full_executable).output().unwrap();
    if !analysis_output.status.success() {
        let e = String::from_utf8(analysis_output.stderr).unwrap();
        panic!("Unable to execute {source:?}. Error output:\n{e}");
    }

    let exec_output = String::from_utf8(analysis_output.stdout).unwrap();

    // chop off all print statements that have nothing to do with ATI
    let start = exec_output.find(ANALYSIS_START).unwrap();
    return exec_output[(start + ANALYSIS_START.len())..].into();
}

/// Checks that the ati stdout stream contains all the expected information,
/// performing a partition comparison, alongside making sure the right number
/// of sites were discovered.
pub fn verify(mut ati_stdout: &str, expected_partition: &HashMap<String, HashMap<String, usize>>) {
    // checks mappings at each site are identical to expected partition
    let mut found_sites = HashSet::new();
    while let Some(end) = ati_stdout.find(SITE_DELIM) {
        let site_info: Vec<_> = ati_stdout[..end].split("\n").collect();
        let mut site_iter = site_info.into_iter().filter(|s| !s.is_empty());
        let site_name = site_iter.next().expect("Found site with no name");
        // dbg!(&site_name);

        assert!(!found_sites.contains(site_name));
        found_sites.insert(site_name);

        // map of var -> id assigned to abstract_type, at this site.
        let mut site_ati_output = HashMap::new();
        for var_info in site_iter {
            if var_info.len() < 3 {
                eprintln!("Found var:type mapping which is malformed: {}", var_info);
                continue;
            }

            let var_split: Vec<_> = var_info.split(":").collect();
            site_ati_output.insert(
                String::from(var_split[0]),
                str::parse::<usize>(var_split[1]).unwrap(),
            );
        }

        // site with name has to exist
        let expected_site = expected_partition.get(site_name);
        assert!(
            expected_site.is_some(),
            "Did not expect {site_name} to exist."
        );

        let expected_site = expected_site.unwrap();
        assert!(expected_site.len() == site_ati_output.len());
        
        let mut expected_to_actual: HashMap<&usize, &usize> = HashMap::new();
        for (var, actual_id) in site_ati_output.iter() {
            let expected_id = expected_site.get(var).expect(&format!("Expected site does not have var: {var}"));
            if let Some(prev_actual_id) = expected_to_actual.get(expected_id) {
                assert_eq!(**prev_actual_id, *actual_id, "Var {var} was found in a wrong set");
            } else {
                expected_to_actual.insert(expected_id, actual_id);
            }
        }

        ati_stdout = &ati_stdout[(end + SITE_DELIM.len())..];
    }


    // found_sites contains no duplicates, so as long as we were able
    assert!(found_sites.len() == expected_partition.len());
}

pub fn delete(exec: &Path) {
    match std::fs::remove_file(exec) {
        Ok(_) => {}
        Err(_) => println!("Unable to remove old file."),
    }
}

pub struct ExpectedSite {
    name: String,
    partition: Vec<(String, usize)>,
}

impl ExpectedSite {
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            partition: Vec::new(),
        }
    }

    pub fn register(mut self, var: &str, comparibility: usize) -> Self {
        self.partition.push((String::from(var), comparibility));
        self
    }

    pub fn register_array(mut self, name: &str, len: usize, comparibility: usize) -> Self {
        let name = String::from(name);
        for i in 0..len {
            self.partition
                .push((format!("{}[{}]", name.clone(), i), comparibility));
        }
        self
    }

    pub fn build(self) -> (String, HashMap<String, usize>) {
        (self.name, HashMap::from_iter(self.partition.into_iter()))
    }
}

#[derive(Default)]
pub struct ExpectedOutput(HashMap<String, HashMap<String, usize>>);
impl ExpectedOutput {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
    pub fn register_site(&mut self, site: ExpectedSite) {
        let (name, site) = site.build();
        self.0.insert(name, site);
    }

    pub fn inner(&self) -> &HashMap<String, HashMap<String, usize>> {
        &self.0
    }
}

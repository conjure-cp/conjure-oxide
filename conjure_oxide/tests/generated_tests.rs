use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use assert_json_diff::assert_json_eq;

use conjure_oxide::ast::Model;

fn integration_test(path: &str) -> Result<(), Box<dyn Error>> {
    println!("Integration test for: {}", path);
    let mut cmd = std::process::Command::new("conjure");
    let output = cmd
        .arg("pretty")
        .arg("--output-format=astjson")
        .arg(format!("{path}/input.essence", path = path))
        .output()?;
    let astjson = String::from_utf8(output.stdout)?;
    let generated_mdl = Model::from_json(&astjson)?;

    let mut expected_str = String::new();
    let mut f = File::open(format!(
        "{path}/input.serialised.expected.json",
        path = path
    ))?;
    f.read_to_string(&mut expected_str)?;
    let mut expected_mdl: Model = serde_json::from_str(&expected_str)?;
    expected_mdl.constraints = Vec::new(); // TODO - remove this line once we parse constraints

    println!("Expected {:#?}", expected_mdl);
    println!("\nActual {:#?}", generated_mdl);

    // assert_json_eq!(serde_json::to_string_pretty(&generated_mdl)?, expected_str); // TODO - replace line once we parse constraints
    assert_eq!(generated_mdl, expected_mdl);

    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/gen_tests.rs"));

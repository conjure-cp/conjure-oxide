use std::error::Error;
use std::fs::File;
use std::io::prelude::*;

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
    let actual_mdl = Model::from_json(&astjson)?;

    let mut expected_str = String::new();
    let mut f = File::open(format!(
        "{path}/input.serialised.expected.json",
        path = path
    ))?;
    f.read_to_string(&mut expected_str)?;
    let expected_mdl: Model = serde_json::from_str(&expected_str)?;

    println!("Expected {:#?}", expected_mdl);
    println!("\nActual {:#?}", actual_mdl);

    assert!(serde_json::to_string_pretty(&actual_mdl)? == expected_str);
    assert!(actual_mdl == expected_mdl);

    // assert!(false);
    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/gen_tests.rs"));

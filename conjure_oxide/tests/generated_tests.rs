use std::env;
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;

use conjure_oxide::ast::Model;

use std::path::Path;

fn main() {
    let file_path = Path::new("/path/to/your/file.txt");
    let base_name = file_path.file_stem().and_then(|stem| stem.to_str());

    match base_name {
        Some(name) => println!("Base name: {}", name),
        None => println!("Could not extract the base name"),
    }
}

fn integration_test(path: &str, essence_base: &str) -> Result<(), Box<dyn Error>> {
    // --------------------------------------------------------------------------------
    // -- parsing the essence file

    // calling conjure to convert Essence to astjson
    let mut cmd = std::process::Command::new("conjure");
    let output = cmd
        .arg("pretty")
        .arg("--output-format=astjson")
        .arg(format!("{path}/{essence_base}.essence"))
        .output()?;
    let stderr_string = String::from_utf8(output.stderr)?;
    assert!(
        stderr_string.is_empty(),
        "conjure's stderr is not empty: {}",
        stderr_string
    );
    let astjson = String::from_utf8(output.stdout)?;

    // "parsing" astjson as Model
    let generated_mdl = Model::from_json(&astjson)?;

    // serialise to file
    let generated_json = serde_json::to_string_pretty(&generated_mdl)?;
    File::create(format!("{path}/{essence_base}.generated.serialised.json"))?
        .write_all(generated_json.as_bytes())?;

    if std::env::var("ACCEPT").map_or(false, |v| v == "true") {
        std::fs::copy(
            format!("{path}/{essence_base}.generated.serialised.json"),
            format!("{path}/{essence_base}.expected.serialised.json"),
        )?;
    }

    // --------------------------------------------------------------------------------
    // -- reading the expected version from the filesystem

    let expected_str =
        std::fs::read_to_string(format!("{path}/{essence_base}.expected.serialised.json"))?;

    let mut expected_mdl: Model = serde_json::from_str(&expected_str)?;
    expected_mdl.constraints = Vec::new(); // TODO - remove this line once we parse constraints

    // --------------------------------------------------------------------------------
    // assert that they are the same model

    assert_eq!(generated_mdl, expected_mdl);

    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/gen_tests.rs"));

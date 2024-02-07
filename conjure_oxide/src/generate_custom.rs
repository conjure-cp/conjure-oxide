// generate_custom.rs with get_example_model function

use std::env;
use std::error::Error;
use std::fs;
use walkdir::WalkDir;
use crate::parse::model_from_json;
use serde_json::Value;

/// Loads corresponding `.essence` file, parses it through `conjure/json`, and returns a `Model`
///
/// # Arguments
///
/// * `filename` - a string slice that holds the name of the file to laod
///
/// # Returns
///
/// Functions returns a `Result<Value, Box<dyn Error>>`
fn get_example_model(filepath: &str) -> Result<Value, Box<dyn Error>> {
    // --------------------------------------------------------------------------------
    // -- parsing the essence file -- copies logic from integration_test()

    // calling conjure to convert Essence to astjson
    let mut cmd = std::process::Command::new("conjure");
    let output = cmd
        .arg("pretty")
        .arg("--output-format=astjson")
        .arg(format!("{filepath}/{essence_base}.essence"))
        .output()?;
    let stderr_string = String::from_utf8(output.stderr)?;
    assert!(
        stderr_string.is_empty(),
        "conjure's stderr is not empty: {}",
        stderr_string
    );

    let astjson = String::from_utf8(output.stdout)?;

    // "parsing" astjson as Model
    let generated_mdl = model_from_json(&astjson)?;

    // a consistent sorting of the keys of json objects
    // only required for the generated version
    // since the expected version will already be sorted
    let generated_json = sort_json_object(&serde_json::to_value(generated_mdl.clone())?);

    // serialise to file
    let generated_json_str = serde_json::to_string_pretty(&generated_json)?;
    File::create(format!("{filepath}/{essence_base}.generated.serialised.json"))?
        .write_all(generated_json_str.as_bytes())?;

    if std::env::var("ACCEPT").map_or(false, |v| v == "true") {
        std::fs::copy(
            format!("{filepath}/{essence_base}.generated.serialised.json"),
            format!("{filepath}/{essence_base}.expected.serialised.json"),
        )?;
    }

    // --------------------------------------------------------------------------------
    // -- reading the expected version from the filesystem

    let expected_str =
        std::fs::read_to_string(format!("{filepath}/{essence_base}.expected.serialised.json"))?;

    let expected_mdl: Model = serde_json::from_str(&expected_str)?;

    // --------------------------------------------------------------------------------
    // assert that they are the same model

    assert_eq!(generated_mdl, expected_mdl);

    Ok((expected_mdl))
}
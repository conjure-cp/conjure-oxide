// generate_custom.rs with get_example_model function

// dependencies
use std::env;
use std::error::Error;
use std::fs::{file, File};
std::io::Write;
use std::path::PathBuf;
use walkdir::WalkDir;
use crate::parse::model_from_json;
use serde_json::Value;

/// Searches recursively in `../tests/integration` folder for an `.essence` file matching the given filename,
/// then uses conjure to process it into astjson, and returns the parsed model.
///
/// # Arguments
///
/// * `filename` - A string slice that holds filename without extension
///
/// # Returns
///
/// Function returns a `Result<Value, Box<dyn Error>>`, where `Value` is the parsed model
fn get_example_model(filename: &str) -> Result<Value, Box<dyn Error>> {
    // define relative path -> integration tests dir
    let test_dir = PathBuf::from("../tests/integration");

    // search matching `.essence` files withing test_dir (similar logic to integration_test() and build.rs)
    for entry in WalkDir::new(&test_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(Result::ok)
    {
        // check if current entry matches filename with `.essence` extension
        let path = entry.path();
        // sanity check to check for extension
        if path.is_file() && path.file_stem() == Some(filename.as_ref()) && path.extension().unwrap_or_default() == "essence" {
            // construct conjure command
            let output = std::process::Command::new("conjure")
                .arg("pretty")
                .arg("--output-format=astjson")
                .arg(path)
                .output()?;

            // validate Conjure's standard error output -> empty
            let stderr_string = String::from_utf8(output.stderr)?;
            assert!(
                stderr_string.is_empty(),
                "STATUS: Conjure's stderr is not empty: {}",
                stderr_string
            );

            // convert Conjure's stdout from bytes to string
            let astjson = String::from_utf8(output.stdout)?;

            // parse AST JSON from desired Model format
            let generate_mdl = model_from_json(&astjson)?;

            // convert and sort model
            let generated_json = sort_json_object(&serde_json::to_value(generated_mdl.clone())?);

            // serialize sorted JSON to pretty string
            let generated_json_str = serde_json::to_string_pretty(&generated_json)?;

            // write serialized JSON to file
            File::create(path.with_extension("generated.serialised.json"))?
                .write_all(generated_json_str.as_bytes())?;

            // if ACCEPT environment var is `true`
            if env::var("ACCEPT").map_or(false, |v| v == "true") {
                fs::copy(
                    path.with_extension("generated.serialised.json"),
                    path.with_extension("expected.serialised.json"),
                )?;
            }

            // read expected JSON model
            let expected_str = fs::read_to_string(path.with_extension("expected.serialised.json"))?;

            // parse expected JSON string into Model format
            let expected_mdl: Value = serde_json::from_str(&expected_str)?;

            assert_eq!(generated_json, expected_mdl);

            // return expected model as final result
            return Ok(expected_mdl);
        }
    }

    // if no matching `.essence` file was found, return error
    Err("ERROR: No matching `.essence` file was found".into())
}



// // --------------------------------------------------------------------------------
/// // -- parsing the essence file -- copies logic from integration_test()

/// // calling conjure to convert Essence to astjson
/// let mut cmd = std::process::Command::new("conjure");
/// let output = cmd
///     .arg("pretty")
///     .arg("--output-format=astjson")
///     .arg(format!("{filepath}/{essence_base}.essence"))
///     .output()?;
/// let stderr_string = String::from_utf8(output.stderr)?;
/// assert!(
///     stderr_string.is_empty(),
///     "conjure's stderr is not empty: {}",
///     stderr_string
/// );

/// let astjson = String::from_utf8(output.stdout)?;

/// // "parsing" astjson as Model
/// let generated_mdl = model_from_json(&astjson)?;

/// // a consistent sorting of the keys of json objects
/// // only required for the generated version
/// // since the expected version will already be sorted
/// let generated_json = sort_json_object(&serde_json::to_value(generated_mdl.clone())?);

/// // serialise to file
/// let generated_json_str = serde_json::to_string_pretty(&generated_json)?;
/// File::create(format!("{filepath}/{essence_base}.generated.serialised.json"))?
///     .write_all(generated_json_str.as_bytes())?;

/// if std::env::var("ACCEPT").map_or(false, |v| v == "true") {
///     std::fs::copy(
///         format!("{filepath}/{essence_base}.generated.serialised.json"),
///         format!("{filepath}/{essence_base}.expected.serialised.json"),
///     )?;
/// }

/// // --------------------------------------------------------------------------------
/// // -- reading the expected version from the filesystem

/// let expected_str =
///     std::fs::read_to_string(format!("{filepath}/{essence_base}.expected.serialised.json"))?;

/// let expected_mdl: Model = serde_json::from_str(&expected_str)?;

/// // --------------------------------------------------------------------------------
/// // assert that they are the same model

/// assert_eq!(generated_mdl, expected_mdl);

/// Ok((expected_mdl))

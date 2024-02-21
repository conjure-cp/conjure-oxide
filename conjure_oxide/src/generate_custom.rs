// generate_custom.rs with get_example_model function

// dependencies
use crate::parse::model_from_json;
use conjure_core::ast::Model;

use std::error::Error;

use std::path::PathBuf;
use walkdir::WalkDir;

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
pub fn get_example_model(filename: &str) -> Result<Model, Box<dyn Error>> {
    // define relative path -> integration tests dir
    let base_dir = "tests/integration";
    let mut essence_path = PathBuf::new();

    // walk through directory tree recursively starting at base
    for entry in WalkDir::new(base_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file()
            && path.extension().map_or(false, |e| e == "essence")
            && path.file_stem() == Some(std::ffi::OsStr::new(filename))
        {
            essence_path = path.to_path_buf();
            break;
        }
    }

    println!("PATH TO FILE: {}", essence_path.display());

    // return error if file not found
    if essence_path.as_os_str().is_empty() {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "ERROR: File not found in any subdirectory",
        )));
    }

    // let path = PathBuf::from(format!("../tests/integration/basic/comprehension{}.essence", filename));
    let mut cmd = std::process::Command::new("conjure");
    let output = cmd
        .arg("pretty")
        .arg("--output-format=astjson")
        .arg(essence_path)
        .output()?;

    // convert Conjure's stdout from bytes to string
    let astjson = String::from_utf8(output.stdout)?;

    println!("ASTJSON: {}", astjson);

    // parse AST JSON from desired Model format
    let generated_mdl = model_from_json(&astjson)?;

    Ok(generated_mdl)
}

/// Searches for an `.essence` file at the given filepath,
/// then uses conjure to process it into astjson, and returns the parsed model.
///
/// # Arguments
///
/// * `filepath` - A string slice that holds the full file path
///
/// # Returns
///
/// Function returns a `Result<Value, Box<dyn Error>>`, where `Value` is the parsed model
pub fn get_example_model_by_path(filepath: &str) -> Result<Model, Box<dyn Error>> {
    let essence_path = PathBuf::from(filepath);

    // return error if file not found
    if essence_path.as_os_str().is_empty() {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "ERROR: File not found in any subdirectory",
        )));
    }

    println!("PATH TO FILE: {}", essence_path.display());

    // Command execution using 'conjure' CLI tool with provided path
    let mut cmd = std::process::Command::new("conjure");
    let output = cmd
        .arg("pretty")
        .arg("--output-format=astjson")
        .arg(&essence_path)
        .output()?;

    // convert Conjure's stdout from bytes to string
    let astjson = String::from_utf8(output.stdout)?;

    println!("ASTJSON: {}", astjson);

    // parse AST JSON into the desired Model format
    let generated_model = model_from_json(&astjson)?;

    Ok(generated_model)
}

/// Recursively sorts the keys of all JSON objects within the provided JSON value.
///
/// serde_json will output JSON objects in an arbitrary key order.
/// this is normally fine, except in our use case we wouldn't want to update the expected output again and again.
/// so a consistent (sorted) ordering of the keys is desirable.
fn sort_json_object(value: &Value) -> Value {
    match value {
        Value::Object(obj) => {
            let mut ordered: Vec<(String, Value)> = obj
                .iter()
                .map(|(k, v)| {
                    if k == "variables" {
                        (k.clone(), sort_json_variables(v))
                    } else {
                        (k.clone(), sort_json_object(v))
                    }
                })
                // .map(|(k, v)| (k.clone(), sort_json_object(v)))
                .collect();
            ordered.sort_by(|a, b| a.0.cmp(&b.0));

            Value::Object(ordered.into_iter().collect())
        }
        Value::Array(arr) => Value::Array(arr.iter().map(sort_json_object).collect()),
        _ => value.clone(),
    }
}

/// Sort the "variables" field by name.
/// We have to do this separately becasue that field is not a JSON object, instead it's an array of tuples.
fn sort_json_variables(value: &Value) -> Value {
    match value {
        Value::Array(vars) => {
            let mut vars_sorted = vars.clone();
            vars_sorted.sort_by(|a, b| {
                let a_obj = &a.as_array().unwrap()[0];
                let a_name: crate::ast::Name = serde_json::from_value(a_obj.clone()).unwrap();

                let b_obj = &b.as_array().unwrap()[0];
                let b_name: crate::ast::Name = serde_json::from_value(b_obj.clone()).unwrap();

                a_name.cmp(&b_name)
            });
            Value::Array(vars_sorted)
        }
        _ => value.clone(),
    }
}

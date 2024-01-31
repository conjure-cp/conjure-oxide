// generate_custom.rs with get_example_model function

use std::error::Error;
use stf::fs::{File, read_dir};
use std::io::prelude::*;
use std::path::Path;
use std::process::Command;
use conjure_oxide::parse::model_from_json;
use serde_json::Value;

fn main() {}

/// Loads corresponding `.essence` file, parses it through `conjure/json`, and returns a `Model`
///
/// # Arguments
///
/// * `filename` - a string slice that holds the name of the file to laod
///
/// # Returns    
///
/// Functions returns a `Result<Model, Box<dyn Error>>`
fn get_example_model(filename: &str) -> Result<Value, Box<dyn Error>> {
    let test_dir = Path::new("tests/integration");
    let essence_path = test_dir.join(format!("{}.essence", filename));

    if !essence_path.exists() {
        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "Essence file not found")));
    }

    // Execute conjure command to convert Essence to json
    let output = Command::new("conjure")
        .arg("pretty")
        .arg("--output-format=astjson")
        .arg(&essence_path)
        .output()?;

    if !output.status.success() {
        let err_msg = format!("Conjure command failed: {}", String::from_utf8_lossy(&output.stderr));
        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, err_msg)));
    }

    let astjson = String::from_utf8_lossy(output.stdout)?;

    // parse astjson as Model
    let model_value: Value = serde_json::from_str(&astjson)?;

    Ok(model_value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_example_model() {
        //
        match get_example_model("xyz") {
            Ok(model) => println!("Model loaded successfully: {:?}", model),
            Err(e) => panic!("Failed to laod and parse the essence file: {:?}", e),
        }
    }
}


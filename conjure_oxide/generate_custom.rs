// generate_custom.rs with get_example_model function

use std::env;
use std::error::Error;
use std::fs;
use walkdir::WalkDir;
use conjure_oxide::parse::model_from_json;
use serde_json::Value;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len()!= 2 {
        eprintln!("Usage: generate_custom <essence_file_name>");
        return Err("Invalid number of arguments".into());
    }

    // name of essence file passed as argument
    let filename = &args[1];

    match get_example_model(filename) {
        Ok(model) => {
            println!("Generated Model: {:?}", model);
            Ok(())
        },
        Err(e) => {
            eprintln!("Error generating model: {}", e);
            Err(e)
        }
    }
}

/// Loads corresponding `.essence` file, parses it through `conjure/json`, and returns a `Model`
///
/// # Arguments
///
/// * `filename` - a string slice that holds the name of the file to laod
///
/// # Returns
///
/// Functions returns a `Result<Value, Box<dyn Error>>`
fn get_example_model(filename: &str) -> Result<Value, Box<dyn Error>> {
    let test_dir = "tests/integration";

    for entry in WalkDir::new(test_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "essence"))
        .filter(|e| e.path().file_name().unwrap() == filename)
    {
        // the path to the directory containing essence file
        let dir_path = entry.path().parent().unwrap().to_str().unwrap();
        // base name (stem) of the essence file without the extension
        let essence_base = entry.path().file_stem().unwrap().to_str().unwrap();

        println!("Found Essence file: {}", entry.path().display());

        // recycle integration_test file
        integration_test(dir_path, essence_base)?;

        // read resulting JSON file that integration_test wrote
        let json_str = fs::read_to_string(format!("{}/{}.generated.serialised.json", dir_path, essence_base))?;
        let model: Value = serde_json::from_str(&json_str)?;

        return Ok(model);
    }

    Err("Essence file not found".into())
}
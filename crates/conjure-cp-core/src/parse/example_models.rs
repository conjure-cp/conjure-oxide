// example_models with get_example_model function

use std::path::PathBuf;

use project_root::get_project_root;
use walkdir::WalkDir;

use crate::Model;
use crate::parse::model_from_json;

/// Searches recursively in `../tests/integration` folder for an `.essence` file matching the given
/// filename, then uses conjure to process it into astjson, and returns the parsed model.
///
/// # Arguments
///
/// * `filename` - A string slice that holds filename without extension
///
/// # Returns
///
/// Function returns a `Result<Value, anyhow::Error>`, where `Value` is the parsed model.
pub fn get_example_model(filename: &str) -> Result<Model, anyhow::Error> {
    // define relative path -> integration tests dir
    let base_dir = get_project_root()?;
    let mut essence_path = PathBuf::new();

    // walk through directory tree recursively starting at base
    for entry in WalkDir::new(base_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|e| e == "essence" || e == "eprime")
            && path.file_stem() == Some(std::ffi::OsStr::new(filename))
        {
            essence_path = path.to_path_buf();
            break;
        }
    }

    //println!("PATH TO FILE: {}", essence_path.display());

    // return error if file not found
    if essence_path.as_os_str().is_empty() {
        return Err(anyhow::Error::new(std::io::Error::new(
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

    //println!("ASTJSON: {}", astjson);

    // parse AST JSON from desired Model format
    let generated_mdl = model_from_json(&astjson, Default::default())?;

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
/// Function returns a `Result<Value, anyhow::Error>`, where `Value` is the parsed model
pub fn get_example_model_by_path(filepath: &str) -> Result<Model, anyhow::Error> {
    let essence_path = PathBuf::from(filepath);

    // return error if file not found
    if essence_path.as_os_str().is_empty() {
        return Err(anyhow::Error::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "ERROR: File not found in any subdirectory",
        )));
    }

    // println!("PATH TO FILE: {}", essence_path.display());

    // Command execution using 'conjure' CLI tool with provided path
    let mut cmd = std::process::Command::new("conjure");
    let output = cmd
        .arg("pretty")
        .arg("--output-format=astjson")
        .arg(&essence_path)
        .output()?;

    // convert Conjure's stdout from bytes to string
    let astjson = String::from_utf8(output.stdout)?;

    // println!("ASTJSON: {}", astjson);

    // parse AST JSON into the desired Model format
    let generated_model = model_from_json(&astjson, Default::default())?;

    Ok(generated_model)
}

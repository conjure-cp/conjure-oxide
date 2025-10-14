use crate::cli::GlobalArgs;
use std::path::PathBuf;
use anyhow::Result;
use conjure_cp::{ast, essence_expr, Model};
use conjure_cp_cli::utils::testing::{read_model_json, save_model_json};
use std::env;
use conjure_cp::parse::tree_sitter::parse_essence_file;
use conjure_cp::context::Context;
use std::sync::{Arc, RwLock};

#[derive(Clone, Debug, clap::Args)]
pub struct Args {
    /// The Essence test directory
    #[arg(default_value = "tests-integration/tests")]
    pub test_directory: PathBuf,
    
    /// Accept current output as expected (update .expected files)
    #[arg(long)]
    pub accept: bool,
}

pub fn run_parse_test_command(global_args: GlobalArgs, parse_test_args: Args) -> Result<()> {

    let test_path = &parse_test_args.test_directory;
    let accept = parse_test_args.accept || env::var("ACCEPT").unwrap_or("false".to_string()) == "true";
    
    if !test_path.exists() {
        anyhow::bail!("Test directory does not exist: {}", test_path.display());
    }

    // Find essence files recursively
    let essence_files = find_essence_files_recursive(test_path)?;

    if essence_files.is_empty() {
        anyhow::bail!("No .essence or .eprime files found in {}", test_path.display());
    }

    println!("Found {} essence files to test", essence_files.len());

    let mut passed = 0;
    let mut failed = 0;

    for essence_file in essence_files {
        let context: Arc<RwLock<Context<'static>>> = Default::default();
        let path = &essence_file.to_string_lossy();
        let test_dir = &essence_file.parent().unwrap().to_string_lossy();
        let essence_base = &essence_file.file_stem().unwrap().to_string_lossy();
        
        // Parse the file
        let parsed_model = match parse_essence_file(path, context.clone()) {
            Ok(model) => {
                save_model_json(&model, &test_dir, &essence_base, "parse")?;
                model
            },
            Err(e ) => {
                println!("{}: Parse error: {}", essence_file.display(), e);
                failed += 1;
                continue;
            }
        };
        // println!("Parsed model: {parsed_model:#?}");

        
        // Create expected file path
        // let expected_file = essence_file.with_extension("expected-parse.serialised.json");
        match read_model_json(&context, test_dir, essence_base, "expected", "parse") {
            Ok(expected_model) => {
                // assert_eq!(parsed_model, expected_model);
                // let model_from_file = read_model_json(&context, test_dir, essence_base, "generated", "parse")?;
                if parsed_model == expected_model {
                    println!("{}: Passed", &essence_file.display());
                    passed += 1;
                }
                else {
                    println!("{}: Parsed model doesn't match expected:", essence_file.display());
                    // println!("Expected: {expected_model:#?}\nParsed: {parsed_model:#?}");
                    // pretty_assertions::assert_eq!(parsed_model, expected_model);
                    failed += 1;
                }
            },
            Err(e) => {
                println!("{}: Expected model could not be found: {}", &essence_file.display(), e);
                failed += 1;
                continue;
            },
        }
        // println!("Expected model: {expected_model:#?}");
        // passed += 1;
    }

    println!("\nParser tests: {} passed, {} failed", passed, failed);
    
    Ok(())
}

fn find_essence_files_recursive(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut essence_files = Vec::new();
    find_essence_files_recursive_helper(dir, &mut essence_files)?;
    Ok(essence_files)
}

fn find_essence_files_recursive_helper(dir: &PathBuf, essence_files: &mut Vec<PathBuf>) -> Result<()> {
    use std::fs;
    
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "essence" || ext == "eprime" {
                    essence_files.push(path);
                }
            }
        } else if path.is_dir() {
            find_essence_files_recursive_helper(&path, essence_files)?;
        }
    }
    
    Ok(())
}

// TODO: check if id is the only thing wrong with the models

// fn id_ignore_check(
//     parsed_model: Model, 
//     expected_model: Model
// ) {
//     for line in parsed_model {}
// }
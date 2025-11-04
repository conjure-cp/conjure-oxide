use std::path::PathBuf;
use anyhow::Result;
use conjure_cp_cli::utils::testing::{read_model_json, save_model_json};
use std::env;
use conjure_cp::parse::tree_sitter::parse_essence_file_native;
use conjure_cp::context::Context;
use std::sync::{Arc, RwLock};
use std::fs;
use serde::Deserialize;

fn copy_generated_to_expected(
    path: &str,
    test_name: &str,
    stage: &str,
    extension: &str,
) -> Result<(), std::io::Error> {
    std::fs::copy(
        format!("{path}/{test_name}.generated-{stage}.{extension}"),
        format!("{path}/{test_name}.expected-{stage}.{extension}"),
    )?;
    Ok(())
}


#[derive(Clone, Debug, clap::Args)]
pub struct Args {
    /// The Essence test directory
    #[arg(default_value = "tests-integration/tests")]
    pub test_directory: PathBuf,
    
    /// Accept current output as expected (update .expected files)
    #[arg(long)]
    pub accept: bool,
}

#[derive(Deserialize)]
struct TestConfig {
    enable_native_parser: Option<bool>
}

// impl Default for TestConfig {
//     fn default() -> Self {
//         Self {
//             extra_rewriter_asserts: vec!["vector_operators_have_partially_evaluated".into()],
//             enable_naive_impl: true,
//             enable_morph_impl: false,
//             enable_rewriter_impl: true,
//             parse_model_default: true,
//             enable_native_parser: true,
//         }
//     }
// }

pub fn run_parse_test_command(parse_test_args: Args) -> Result<()> {

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
        
        let use_native_parser: bool =
            if let Ok(config_contents) = fs::read_to_string(format!("{}/config.toml", test_dir)) {
                match toml::from_str::<TestConfig>(&config_contents) {
                    Ok(cfg) => cfg.enable_native_parser.unwrap_or(true),
                    Err(e) => {
                        println!("{}: Failed to parse config.toml: {}", essence_file.display(), e);
                        true
                    }
                }

            } else {
                true
            };
        
        if !use_native_parser {
            println!("{}: Skipped because native parser disabled in config.toml", essence_file.display());
            continue;
        }

        // Parse the file
        let parsed_model = match std::panic::catch_unwind(|| parse_essence_file_native(path, context.clone())) {
            Ok(Ok(model)) => {
                save_model_json(&model, &test_dir, &essence_base, "parse")?;
                model
            },
            Ok(Err(e)) => {
                println!("{}: Parse error: {}", essence_file.display(), e);
                failed += 1;
                continue;
            },
            Err(payload) => {
                let panic_msg = if let Some(s) = (&payload).downcast_ref::<&'static str>() {
                    s.to_string()
                } else if let Some(s) = payload.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Parser panicked: non-string payload".to_string() 
                };
                println!("{}: Parser panicked: {}", essence_file.display(), panic_msg);
                failed += 1;
                continue;
            }
        };
                        
        match read_model_json(&context, test_dir, essence_base, "expected", "parse") {
            Ok(_) => {
                // assert_eq!(parsed_model, expected_model);
                // let model_from_file = read_model_json(&context, test_dir, essence_base, "generated", "parse")?;
                match compare_json_ignoring_ids(test_dir, essence_base) {
                    Ok(equal) => {
                        if equal {
                            println!("{}: Passed", essence_file.display());
                            passed += 1;
                        } else {
                            if accept {
                                match copy_generated_to_expected(&test_dir, &essence_base, "parse", "serialised.json") {
                                    Ok(_) => passed += 1,
                                    Err(e) => {
                                        println!("Failed to save expected model for {}: {}", essence_base, e);
                                        failed += 1;
                                    }
                                }
                            } else {
                                println!("{}: Parsed model doesn't match expected", essence_file.display());
                                failed += 1;
                            }
                        }
                    }
                    Err(e) => {
                        println!("{}: Error comparing expected and generated results: {}", essence_file.display(), e);
                        failed += 1;
                    }
                }
            },
            Err(e) => {
                println!("{}: Expected model could not be found: {}", essence_file.display(), e);
                failed += 1;
                continue;
            },
        }
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

fn compare_json_ignoring_ids(
    test_dir: &str, 
    base: &str
) -> Result<bool> {
    let gen_path = format!("{}/{}.generated-parse.serialised.json", test_dir, base);
    let exp_path = format!("{}/{}.expected-parse.serialised.json", test_dir, base);

    let gen_raw = match fs::read_to_string(&gen_path) {
        Ok(s) => s,
        Err(e) => {
            println!("Error reading {}: {}", gen_path, e);
            return Err(anyhow::anyhow!("Error reading {}: {}", gen_path, e));
        }
    };

    let exp_raw = match fs::read_to_string(&exp_path) {
        Ok(s) => s,
        Err(e) => {
            println!("Error reading {}: {}", exp_path, e);
            return Err(anyhow::anyhow!("Error reading {}: {}", exp_path, e));
        }
    };

    let gen_val: serde_json::Value = serde_json::from_str(&gen_raw).
        map_err(|e| anyhow::anyhow!("Failed to parse JSON {}: {}", gen_path, e))?;
    let exp_val: serde_json::Value = serde_json::from_str(&exp_raw).
        map_err(|e| anyhow::anyhow!("Failed to parse JSON {}: {}", exp_path, e))?;
    
    let gen_string = serde_json::to_string_pretty(&gen_val)?;
    let exp_string = serde_json::to_string_pretty(&exp_val)?;

    if gen_string == exp_string {
        return Ok(true);
    }

    let gen_lines: Vec<&str> = gen_string.lines().collect();
    let exp_lines: Vec<&str> = exp_string.lines().collect();
    let max = std::cmp::min(gen_lines.len(), exp_lines.len());

    let ignore_words = vec![
        "\"Reference\":",
        "\"id\":",
        "\"parent\":"
    ];

    for i in 0..max {
        if ignore_words.iter().any(|w| gen_lines[i].contains(w) && exp_lines[i].contains(w)) {
            continue;
        }

        if gen_lines[i] != exp_lines[i] {
            println!("\nFirst difference found at line {}", i);
            println!("Expected: {}", exp_lines[i]);
            println!("Generated: {}\n", gen_lines[i]);
            return Ok(false);
        }
    }

    if gen_lines.len() != exp_lines.len() {
        println!("Number of lines different from expected: expected {} lines, generated {} lines", exp_lines.len(), gen_lines.len());
        return Ok(false);
    }

    Ok(true)
}
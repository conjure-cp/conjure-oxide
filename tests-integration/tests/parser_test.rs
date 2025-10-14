use pretty_assertions::assert_eq;
use std::borrow::Cow;
use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use conjure_cp::parse::tree_sitter::parse_essence_file;
use conjure_cp::context::Context;
use std::sync::Arc;
use std::sync::RwLock;

pub fn parser_test(test_dir: &str) -> Result<(), Box<dyn Error>> {
    let accept = env::var("ACCEPT").unwrap_or("false".to_string()) == "true";

    // Convert test directory to a PathBuf
    let test_path = PathBuf::from(test_dir);
    assert!(
        test_path.exists(),
        "Test directory not found: {test_path:?}"
    );

    let essence_files: Vec<_> = fs::read_dir(&test_path)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();

            if path.extension()? == "essence" || path.extension()? == "eprime" {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    assert!(!essence_files.is_empty(), "No .essence or .eprime files found in {test_path:?}");

    for essence_file in essence_files {
        let essence_content = fs::read_to_string(&essence_file);

        let context: Arc<RwLock<Context<'static>>> = Default::default();
        let parse_result = parse_essence_file(&essence_file.to_string_lossy(), context);

        let expected_parse_path = test_path.join("input.generated-parse.serialised.json");
        let actual_parse_output = match parse_result {
            Ok(ast) => format!("Parse successful:\n{:#?}", ast),
            Err(e) => format!("Parse error: {}", e),
        };

        if accept {
            update_file(expected_parse_path, Cow::Borrowed(&actual_parse_output))?;
        } else {
            let expected_parse = if expected_parse_path.exists() {
                fs::read_to_string(&expected_parse_path)?
            } else {
                String::new()
            };

            assert_eq!(expected_parse, actual_parse_output, "Parse result mismatch for {}", essence_file.display());
        }
    }

    Ok(())

    // // Get paths
    // let script_path = test_path.join("run.sh");
    // assert!(
    //     script_path.exists(),
    //     "Test script not found: {script_path:?}"
    // );
    // let expected_output_path = test_path.join("input.generated-parse.serialised.json");
    // // let expected_error_path = test_path.join("stderr.expected");

    // // Get conjure_oxide binary path from test binary path:
    // // The test binary is at target/XX/deps/TESTPROGNAME and conjure_oxide is at target/XX/conjure-oxide
    // // so from test binary, need to go up two directories and add 'conjure-oxide'
    // let mut conjure_oxide_path = env::current_exe().unwrap();
    // conjure_oxide_path.pop();
    // conjure_oxide_path.pop();
    // conjure_oxide_path.push("conjure-oxide");

    // // Modify PATH so run.sh can find conjure_oxide
    // let mut path_var = env::var("PATH").unwrap_or_else(|_| "".to_string());
    // let conjure_dir = conjure_oxide_path.parent().unwrap().to_str().unwrap();
    // path_var = format!("{conjure_dir}:{path_var}");

    // // Execute the test script in the correct directory
    // let output: Output = Command::new("sh")
    //     .arg("run.sh")
    //     .current_dir(&test_path)
    //     .env("PATH", path_var)
    //     .output()?;

    // // Convert captured output/error to string
    // let actual_output = String::from_utf8_lossy(&output.stdout);
    // let actual_error = String::from_utf8_lossy(&output.stderr);

    // if accept {
    //     // Overwrite expected files
    //     update_file(expected_output_path, actual_output)?;
    //     // update_file(expected_error_path, actual_error)?;
    // } else {
    //     // Compare results
    //     let expected_output = if expected_output_path.exists() {
    //         fs::read_to_string(&expected_output_path)?
    //     } else {
    //         String::new()
    //     };
    //     // let expected_error = if expected_error_path.exists() {
    //     //     fs::read_to_string(&expected_error_path)?
    //     // } else {
    //     //     String::new()
    //     // };

    //     assert_eq!(expected_output, actual_output, "Standard output mismatch");
    //     // assert_eq!(expected_error, actual_error, "Standard error mismatch");
    // }

    // Ok(())
}

fn update_file(
    expected_file_path: PathBuf,
    actual_output: Cow<'_, str>,
) -> Result<(), Box<dyn Error>> {
    if expected_file_path.exists() {
        fs::remove_file(&expected_file_path)?;
    }
    if !actual_output.trim().is_empty() {
        fs::File::create(&expected_file_path)?;
        fs::write(&expected_file_path, actual_output.as_bytes())?;
    }
    Ok(())
}

#[test]
fn assert_conjure_present() {
    conjure_cp_cli::find_conjure::conjure_executable().unwrap();
}

// include!(concat!(env!("OUT_DIR"), "/gen_tests_parser.rs"));

#[test]
fn test_simple_parser_success() {
    let temp_dir = std::env::temp_dir().join("simple_parser_test");
    std::fs::create_dir_all(&temp_dir).unwrap();
    
    let test_essence = temp_dir.join("test.essence");
    std::fs::write(&test_essence, r#"
        find x : int(1..3)
        such that x <= 2
    "#).unwrap();
    
    let context: Arc<RwLock<Context<'static>>> = Default::default();
    let parse_result = parse_essence_file(&test_essence.to_string_lossy(), context);
    
    std::fs::remove_dir_all(&temp_dir).unwrap();
    
    assert!(parse_result.is_ok(), "Parser should succeed on valid Essence file");
}

#[test]
fn test_parser_test_function() {
    // Create a temporary test directory with a simple essence file
    let temp_dir = std::env::temp_dir().join("parser_test_temp");
    std::fs::create_dir_all(&temp_dir).unwrap();
    
    // Create a simple essence file for testing
    let test_essence = temp_dir.join("test.essence");
    std::fs::write(&test_essence, r#"
        find x : int(1..3)
        such that x <= 2
    "#).unwrap();
    
    // Test your parser function
    let result = parser_test(&temp_dir.to_string_lossy());
    
    // Clean up
    std::fs::remove_dir_all(&temp_dir).unwrap();
    
    // Check if it worked
    match result {
        Ok(_) => println!("✓ Parser test function works!"),
        Err(e) => panic!("✗ Parser test failed: {}", e),
    }
}

#[test] 
fn test_single_file_parsing() {
    // Test just the parsing part directly
    use conjure_cp::parse::tree_sitter::parse_essence_file;
    use conjure_cp::context::Context;
    use std::sync::{Arc, RwLock};
    
    let temp_file = std::env::temp_dir().join("single_test.essence");
    std::fs::write(&temp_file, r#"
        find x : int(1..3)
        such that x <= 2
    "#).unwrap();
    
    let context: Arc<RwLock<Context<'static>>> = Default::default();
    let parse_result = parse_essence_file(&temp_file.to_string_lossy(), context);
    
    std::fs::remove_file(&temp_file).unwrap();
    
    match parse_result {
        Ok(ast) => {
            println!("✓ Single file parsing works!");
            println!("AST: {:#?}", ast);
        }
        Err(e) => panic!("✗ Single file parsing failed: {}", e),
    }
}
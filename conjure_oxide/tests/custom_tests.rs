use std::fs;
use std::process::{Command, Output};
use std::path::Path;
use std::error::Error;
use pretty_assertions::assert_eq;
use std::env;

pub fn custom_test(test_dir: &str) -> Result<(), Box<dyn Error>> {
    let accept = env::var("ACCEPT").unwrap_or("false".to_string()) == "true";
    // let verbose = env::var("VERBOSE").unwrap_or("false".to_string()) == "true";

    let test_path = Path::new(test_dir);

    // Locate the shell script
    let script_path = test_path.join("run.sh");
    if !script_path.exists() {
        return Err(format!("Test script not found: {:?}", script_path).into());
    }

    // Locate expected output and error files
    let expected_output_path = test_path.join("stdout.expected");
    let expected_error_path = test_path.join("stderr.expected");

    if !expected_output_path.exists() || !expected_error_path.exists() {
        return Err("Expected output or error file is missing".into());
    }

    // Execute the test script
    let output: Output = Command::new("sh")
        .arg(script_path.to_str().unwrap())
        .output()?;

    // Convert captured output/error to string
    let actual_output = String::from_utf8_lossy(&output.stdout);
    let actual_error = String::from_utf8_lossy(&output.stderr);
    
    if accept {
        fs::write(&expected_output_path, actual_output.as_bytes())?;
        fs::write(&expected_error_path, actual_error.as_bytes())?;
    } else {
        // Read expected output and error
        let expected_output = fs::read_to_string(&expected_output_path)?;
        let expected_error = fs::read_to_string(&expected_error_path)?;

        // Compare results
        assert_eq!(expected_output, actual_output, "Standard output ");
        assert_eq!(expected_error, actual_error, "Standard error ");
    }

    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/gen_tests_custom.rs"));
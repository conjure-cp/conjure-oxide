use pretty_assertions::assert_eq;
use std::borrow::Cow;
use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

pub fn custom_test(test_dir: &str) -> Result<(), Box<dyn Error>> {
    let accept = env::var("ACCEPT").unwrap_or("false".to_string()) == "true";

    // Convert test directory to a PathBuf
    let test_path = PathBuf::from(test_dir);
    assert!(
        test_path.exists(),
        "Test directory not found: {test_path:?}"
    );

    // Get paths
    let script_path = test_path.join("run.sh");
    assert!(
        script_path.exists(),
        "Test script not found: {script_path:?}"
    );
    let expected_output_path = test_path.join("stdout.expected");
    let expected_error_path = test_path.join("stderr.expected");

    // Get conjure_oxide binary path from test binary path:
    // The test binary is at target/XX/deps/TESTPROGNAME and conjure_oxide is at target/XX/conjure-oxide
    // so from test binary, need to go up two directories and add 'conjure-oxide'
    let mut conjure_oxide_path = env::current_exe().unwrap();
    conjure_oxide_path.pop();
    conjure_oxide_path.pop();
    conjure_oxide_path.push("conjure-oxide");

    // Modify PATH so run.sh can find conjure_oxide
    let mut path_var = env::var("PATH").unwrap_or_else(|_| "".to_string());
    let conjure_dir = conjure_oxide_path.parent().unwrap().to_str().unwrap();
    path_var = format!("{conjure_dir}:{path_var}");

    // Execute the test script in the correct directory
    let output: Output = Command::new("sh")
        .arg("run.sh")
        .current_dir(&test_path)
        .env("PATH", path_var)
        .output()?;

    // Convert captured output/error to string
    let actual_output = String::from_utf8_lossy(&output.stdout);
    let actual_error = String::from_utf8_lossy(&output.stderr);

    if accept {
        // Overwrite expected files
        update_file(expected_output_path, actual_output)?;
        update_file(expected_error_path, actual_error)?;
    } else {
        // Compare results
        let expected_output = if expected_output_path.exists() {
            fs::read_to_string(&expected_output_path)?
        } else {
            String::new()
        };
        let expected_error = if expected_error_path.exists() {
            fs::read_to_string(&expected_error_path)?
        } else {
            String::new()
        };

        assert_eq!(expected_output, actual_output, "Standard output mismatch");
        assert_eq!(expected_error, actual_error, "Standard error mismatch");
    }

    Ok(())
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

include!(concat!(env!("OUT_DIR"), "/gen_tests_custom.rs"));

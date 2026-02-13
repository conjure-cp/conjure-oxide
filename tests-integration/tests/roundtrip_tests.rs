use conjure_cp::Model;
use conjure_cp::context::Context;
use conjure_cp::parse::tree_sitter::errors::ParseErrorCollection;
use conjure_cp::parse::tree_sitter::{parse_essence_file, parse_essence_file_native};
use conjure_cp_cli::utils::testing::{read_model_json, save_model_json};

use std::env;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

use serde::Deserialize;

use std::io::Write;

// Allows for different configurations of parsers per test
#[derive(Deserialize)]
struct TestConfig {
    parsers: Vec<String>,
}

// The default test configuration is both enabled
impl Default for TestConfig {
    fn default() -> Self {
        Self {
            parsers: vec![format!("legacy"), format!("native")],
        }
    }
}

// Designed to test if an Essence feature can be parsed correctly into the AST and complete a roundtrip
// Does not consider rewriting or solving
fn roundtrip_test(path: &str, filename: &str, extension: &str) -> Result<(), Box<dyn Error>> {
    // Reads in a config.toml in the test directory
    let file_config: TestConfig =
        if let Ok(config_contents) = fs::read_to_string(format!("{path}/config.toml")) {
            toml::from_str(&config_contents).unwrap()
        } else {
            Default::default()
        };
    // Runs native parser
    if file_config.parsers.contains(&format!("native")) {
        let new_filename = filename.to_owned() + "-native";
        roundtrip_test_inner(
            path,
            &filename,
            &new_filename,
            extension,
            parse_essence_file_native,
        )?;
    }
    // Runs legacy Conjure parser
    if file_config.parsers.contains(&format!("legacy")) {
        let new_filename = filename.to_owned() + "-legacy";
        roundtrip_test_inner(
            path,
            &filename,
            &new_filename,
            extension,
            parse_essence_file,
        )?;
    }
    Ok(())
}

// Runs the test for either parser
fn roundtrip_test_inner(
    path: &str,
    input_filename: &str,
    output_filename: &str,
    extension: &str,
    parse: fn(&str, Arc<RwLock<Context<'static>>>) -> Result<Model, Box<ParseErrorCollection>>,
) -> Result<(), Box<dyn Error>> {
    /*
    Parses Essence file
     | If valid
        Saves generated AST model JSON
        Saves generated Essence

        Compares expected and generated AST model JSON
        Compares expected and generated Essence

        Parses generated Essence back to being a model
        Saves new model as Essence (generated2)
        Compare initally generated Essence with newly generated Essence

    | If invalid
        Saves EssenceParseError
        Compares expected and generated errors
    */

    let accept = env::var("ACCEPT").unwrap_or("false".to_string()) == "true";

    let file_path = format!("{path}/{input_filename}.{extension}");
    let context: Arc<RwLock<Context<'static>>> = Default::default();

    let initial_parse = parse(&file_path, context.clone());
    match initial_parse {
        Ok(initial_model) => {
            save_model_json(&initial_model, path, output_filename, "parse")?;
            save_essence(&initial_model, path, output_filename, "generated")?;

            // When ACCEPT = true, copy over generated to expected
            if accept {
                std::fs::copy(
                    format!("{path}/{output_filename}.generated-parse.serialised.json"),
                    format!("{path}/{output_filename}.expected-parse.serialised.json"),
                )?;
                std::fs::copy(
                    format!("{path}/{output_filename}.generated-essence.essence"),
                    format!("{path}/{output_filename}.expected-essence.essence"),
                )?;
            }

            // Ensures ACCEPT=true has been run at least once
            if !accept
                && !Path::new(&format!(
                    "{path}/{output_filename}.expected-parse.serialised.json"
                ))
                .exists()
            {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Expected output file not found: Run with ACCEPT=true"),
                )));
            }

            // Compare the expected and generated model
            let expected_model =
                read_model_json(&context, path, output_filename, "expected", "parse")?;
            let generated_model =
                read_model_json(&context, path, output_filename, "generated", "parse")?;
            assert_eq!(generated_model, expected_model);

            // Compares essence files
            let expected_essence = fs::read_to_string(&format!(
                "{path}/{output_filename}.expected-essence.essence"
            ))?;
            let generated_essence = fs::read_to_string(&format!(
                "{path}/{output_filename}.generated-essence.essence"
            ))?;
            assert_eq!(expected_essence, generated_essence);

            // Compares roundtrip
            let new_model = parse(
                &format!("{path}/{output_filename}.generated-essence.essence"),
                context.clone(),
            )?;
            save_essence(&new_model, path, output_filename, "generated2")?;
            let new_generated_essence = fs::read_to_string(&format!(
                "{path}/{output_filename}.generated2-essence.essence"
            ))?;
            assert_eq!(generated_essence, new_generated_essence);
        }

        Err(parse_error) => {
            save_parse_error(&parse_error, path, output_filename, "generated")?;

            // When ACCEPT = true, copy over generated to expected
            if accept {
                std::fs::copy(
                    format!("{path}/{output_filename}.generated-error.txt"),
                    format!("{path}/{output_filename}.expected-error.txt"),
                )?;
            }

            // Ensures ACCEPT=true has been run at least once
            if !accept
                && !Path::new(&format!("{path}/{output_filename}.expected-error.txt")).exists()
            {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Expected output file not found: Run with ACCEPT=true"),
                )));
            }

            let expected_error =
                fs::read_to_string(&format!("{path}/{output_filename}.expected-error.txt"))?;
            let generated_error =
                fs::read_to_string(&format!("{path}/{output_filename}.generated-error.txt"))?;
            assert_eq!(expected_error, generated_error);
        }
    }

    Ok(())
}

/* Saves a model as an Essence file */
fn save_essence(
    model: &Model,
    path: &str,
    test_name: &str,
    model_type: &str,
) -> Result<(), std::io::Error> {
    let filename = format!("{path}/{test_name}.{model_type}-essence.essence");
    let mut file = fs::File::create(&filename)?;
    write!(file, "{}", model)?;
    Ok(())
}

/* Saves a error message as a text file */
fn save_parse_error(
    error: &ParseErrorCollection,
    path: &str,
    test_name: &str,
    model_type: &str,
) -> Result<(), std::io::Error> {
    let filename = format!("{path}/{test_name}.{model_type}-error.txt");
    let mut file = fs::File::create(&filename)?;
    write!(file, "{}", error)?;
    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/gen_tests_roundtrip.rs"));

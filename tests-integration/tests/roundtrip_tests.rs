use conjure_cp::Model;
use conjure_cp::ast::SerdeModel;
use conjure_cp::context::Context;
use conjure_cp::parse::tree_sitter::errors::ParseErrorCollection;
use conjure_cp::parse::tree_sitter::{parse_essence_file, parse_essence_file_native};
use conjure_cp::settings::Parser;
use conjure_cp_cli::utils::testing::serialize_model;
use tests_integration::TestConfig;

use std::env;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

use std::io::Write;

/// Parser function used by roundtrip tests.
type ParseFn = fn(&str, Arc<RwLock<Context<'static>>>) -> Result<Model, Box<ParseErrorCollection>>;

/// Runs a roundtrip parse test for one input model using the parsers configured in `config.toml`.
fn roundtrip_test(path: &str, filename: &str, extension: &str) -> Result<(), Box<dyn Error>> {
    let accept = env::var("ACCEPT").unwrap_or("false".to_string()) == "true";

    let file_config: TestConfig =
        if let Ok(config_contents) = fs::read_to_string(format!("{path}/config.toml")) {
            toml::from_str(&config_contents).unwrap()
        } else {
            Default::default()
        };

    if accept {
        clean_test_dir_for_accept(path)?;
    }

    let parsers = file_config
        .configured_parsers()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;

    for parser in parsers {
        let case_name = parser.to_string();
        let parse = match parser {
            Parser::TreeSitter => parse_essence_file_native,
            Parser::ViaConjure => parse_essence_file,
        };
        roundtrip_test_inner(path, filename, &case_name, extension, parse)?;
    }
    Ok(())
}

/// Removes generated and expected artefacts for a roundtrip test directory when `ACCEPT=true`.
///
/// Keeps source model files (`.essence`) and `config.toml`. Nested directories are not removed,
/// because each nested test directory performs its own cleanup when executed.
fn clean_test_dir_for_accept(path: &str) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let entry_path = entry.path();

        if entry_path.is_dir() {
            continue;
        }

        let keep = if file_name == "config.toml" {
            true
        } else {
            let is_model_file = entry_path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext == "essence");
            let is_generated_or_expected =
                file_name.contains(".generated") || file_name.contains(".expected");
            is_model_file && !is_generated_or_expected
        };

        if keep {
            continue;
        }

        std::fs::remove_file(entry_path)?;
    }

    Ok(())
}

/// Runs the roundtrip pipeline for a single parser case.
///
/// Algorithm sketch:
/// 1. Parse the input model file.
/// 2. If parsing succeeds:
/// 3. Save generated model JSON and generated Essence output.
/// 4. If `ACCEPT=true`, copy generated outputs to expected outputs.
/// 5. Load and compare generated vs expected model JSON.
/// 6. Load and compare generated vs expected Essence output.
/// 7. Parse generated Essence again, re-emit Essence, and assert roundtrip stability.
/// 8. If parsing fails:
/// 9. Save generated parse error output.
/// 10. If `ACCEPT=true`, copy generated error output to expected error output.
/// 11. Load and compare generated vs expected error output.
fn roundtrip_test_inner(
    path: &str,
    input_filename: &str,
    case_name: &str,
    extension: &str,
    parse: ParseFn,
) -> Result<(), Box<dyn Error>> {
    let accept = env::var("ACCEPT").unwrap_or("false".to_string()) == "true";

    let file_path = format!("{path}/{input_filename}.{extension}");
    let context: Arc<RwLock<Context<'static>>> = Default::default();

    let initial_parse = parse(&file_path, context.clone());
    match initial_parse {
        Ok(initial_model) => {
            save_roundtrip_model_json(&initial_model, path, case_name, "generated")?;
            save_essence(&initial_model, path, case_name, "generated")?;

            if accept {
                std::fs::copy(
                    roundtrip_model_json_path(path, case_name, "generated"),
                    roundtrip_model_json_path(path, case_name, "expected"),
                )?;
                std::fs::copy(
                    roundtrip_essence_path(path, case_name, "generated"),
                    roundtrip_essence_path(path, case_name, "expected"),
                )?;
            }

            if !accept
                && !Path::new(&roundtrip_model_json_path(path, case_name, "expected")).exists()
            {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Expected output file not found: Run with ACCEPT=true".to_string(),
                )));
            }

            let expected_model = read_roundtrip_model_json(&context, path, case_name, "expected")?;

            let generated_model =
                read_roundtrip_model_json(&context, path, case_name, "generated")?;
            assert_eq!(generated_model, expected_model);

            let expected_essence =
                fs::read_to_string(roundtrip_essence_path(path, case_name, "expected"))?;
            let generated_essence =
                fs::read_to_string(roundtrip_essence_path(path, case_name, "generated"))?;
            assert_eq!(expected_essence, generated_essence);

            let new_model = parse(
                &roundtrip_essence_path(path, case_name, "generated"),
                context.clone(),
            )?;
            save_essence(&new_model, path, case_name, "generated2")?;
            let new_generated_essence =
                fs::read_to_string(roundtrip_essence_path(path, case_name, "generated"))?;
            assert_eq!(generated_essence, new_generated_essence);
        }

        Err(parse_error) => {
            save_parse_error(&parse_error, path, case_name, "generated")?;

            if accept {
                std::fs::copy(
                    roundtrip_error_path(path, case_name, "generated"),
                    roundtrip_error_path(path, case_name, "expected"),
                )?;
            }

            if !accept && !Path::new(&roundtrip_error_path(path, case_name, "expected")).exists() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Expected output file not found: Run with ACCEPT=true".to_string(),
                )));
            }

            let expected_error =
                fs::read_to_string(roundtrip_error_path(path, case_name, "expected"))?;
            let generated_error =
                fs::read_to_string(roundtrip_error_path(path, case_name, "generated"))?;
            assert_eq!(expected_error, generated_error);
        }
    }

    Ok(())
}

/// Returns the roundtrip model JSON path for a parser case and model type.
fn roundtrip_model_json_path(path: &str, case_name: &str, file_type: &str) -> String {
    format!("{path}/{case_name}.{file_type}.serialised.json")
}

/// Returns the roundtrip Essence path for a parser case and model type.
fn roundtrip_essence_path(path: &str, case_name: &str, file_type: &str) -> String {
    format!("{path}/{case_name}.{file_type}.essence")
}

/// Returns the roundtrip parser-error path for a parser case and model type.
fn roundtrip_error_path(path: &str, case_name: &str, file_type: &str) -> String {
    format!("{path}/{case_name}.{file_type}-error.txt")
}

/// Serialises and writes a generated model snapshot for roundtrip comparison.
fn save_roundtrip_model_json(
    model: &Model,
    path: &str,
    case_name: &str,
    file_type: &str,
) -> Result<(), std::io::Error> {
    let serialised = serialize_model(model).map_err(std::io::Error::other)?;
    fs::write(
        roundtrip_model_json_path(path, case_name, file_type),
        serialised,
    )?;
    Ok(())
}

/// Reads and initialises a saved roundtrip model snapshot.
fn read_roundtrip_model_json(
    context: &Arc<RwLock<Context<'static>>>,
    path: &str,
    case_name: &str,
    file_type: &str,
) -> Result<Model, std::io::Error> {
    let serialised = fs::read_to_string(roundtrip_model_json_path(path, case_name, file_type))?;
    let serde_model: SerdeModel =
        serde_json::from_str(&serialised).map_err(std::io::Error::other)?;
    serde_model
        .initialise(context.clone())
        .ok_or_else(|| std::io::Error::other("failed to initialise parsed SerdeModel"))
}

/// Saves a model as an Essence file.
fn save_essence(
    model: &Model,
    path: &str,
    test_name: &str,
    file_type: &str,
) -> Result<(), std::io::Error> {
    let filename = roundtrip_essence_path(path, test_name, file_type);
    let mut file = fs::File::create(&filename)?;
    write!(file, "{}", model)?;
    Ok(())
}

/// Saves a parse error message as a text file.
fn save_parse_error(
    error: &ParseErrorCollection,
    path: &str,
    test_name: &str,
    file_type: &str,
) -> Result<(), std::io::Error> {
    let filename = roundtrip_error_path(path, test_name, file_type);
    let mut file = fs::File::create(&filename)?;
    write!(file, "{}", error)?;
    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/gen_tests_roundtrip.rs"));

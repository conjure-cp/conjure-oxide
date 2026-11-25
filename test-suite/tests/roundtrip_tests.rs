use conjure_cp::Model;
use conjure_cp::context::Context;
use conjure_cp::instantiate::{instantiate_model, validate_instantiation_conditions};
use conjure_cp::parse::tree_sitter::errors::InstantiateModelError;
use conjure_cp::parse::tree_sitter::errors::ParseErrorCollection;
use conjure_cp::parse::tree_sitter::parse_essence_file_native;
use conjure_cp_cli::utils::testing::{
    DEFAULT_TEXT_SNAPSHOT_CHARACTER_LIMIT, serialize_model, truncate_to_first_chars,
};
use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;
use test_suite::AcceptMode;
use test_suite::golden_files::assert_no_redundant_expected_files;

/// Parser function used by roundtrip tests.
type ParseFn = fn(&str, Arc<RwLock<Context<'static>>>) -> Result<Model, Box<ParseErrorCollection>>;

/// Runs a roundtrip parse test for one input model using the parsers configured in `config.toml`.
fn roundtrip_test(path: &str, filename: &str, extension: &str) -> Result<(), Box<dyn Error>> {
    let accept = AcceptMode::from_env().accepts_outputs();

    let param_file = fs::read_dir(path).ok().and_then(|entries| {
        entries
            .filter_map(|entry| entry.ok())
            .find(|entry| entry.path().extension().is_some_and(|ext| ext == "param"))
            .map(|entry| entry.file_name().to_string_lossy().to_string())
    });

    if accept {
        clean_test_dir_for_accept(path)?;
    }

    let mut allowed_expected_files = BTreeSet::new();

    let parse = parse_essence_file_native;
    allowed_expected_files.extend(roundtrip_test_inner(
        path,
        filename,
        extension,
        parse,
        param_file.as_deref(),
    )?);

    assert_no_redundant_expected_files(Path::new(path), &allowed_expected_files, None)?;

    Ok(())
}

/// Removes generated and expected artefacts for a roundtrip test directory when accept mode is enabled.
///
/// Keeps source model files (`.essence`, `.param`) and `config.toml`. Nested directories are not removed,
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

        let keep = if file_name == "config.toml" || file_name == "notes.txt" {
            true
        } else {
            let is_model_file = entry_path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext == "essence" || ext == "param");
            let is_generated_or_expected =
                matches!(file_name.as_ref(), "generated.essence" | "expected.essence");
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
/// 4. If accept mode is enabled, copy generated outputs to expected outputs.
/// 5. Load and compare generated vs expected model JSON.
/// 6. Load and compare generated vs expected Essence output.
/// 7. Parse generated Essence again, re-emit Essence, and assert roundtrip stability.
/// 8. If parsing fails:
/// 9. Save generated parse error output.
/// 10. If accept mode is enabled, copy generated error output to expected error output.
/// 11. Load and compare generated vs expected error output.
fn roundtrip_test_inner(
    path: &str,
    input_filename: &str,
    extension: &str,
    parse: ParseFn,
    param_file: Option<&str>,
) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let accept = AcceptMode::from_env().accepts_outputs();

    let file_path = format!("{path}/{input_filename}.{extension}");
    let context: Arc<RwLock<Context<'static>>> = Default::default();

    // let problem_model = parse(&global_args, Arc::clone(&context), essence_file_name)?;
    let problem_model = parse(&file_path, context.clone());

    let initial_parse = match problem_model {
        Ok(problem_model) => match param_file {
            Some(param_file_name) => {
                let param_file_path = format!("{path}/{param_file_name}");
                let param_model = parse(&param_file_path, context.clone());
                match param_model {
                    Ok(param_model) => instantiate_model(problem_model, param_model).map_err(|e| {
                        Box::new(ParseErrorCollection::InstantiateModel(
                            InstantiateModelError {
                                msg: format!("{e}"),
                            },
                        ))
                    }),
                    Err(e) => Err(e),
                }
            }
            None => {
                let mut problem_model = problem_model;
                validate_instantiation_conditions(&mut problem_model).map_err(|e| {
                    Box::new(ParseErrorCollection::InstantiateModel(
                        InstantiateModelError {
                            msg: format!("{e}"),
                        },
                    )) as Box<dyn Error>
                })?;
                Ok(problem_model)
            }
        },
        Err(e) => Err(e),
    };
    match initial_parse {
        Ok(initial_model) => {
            save_roundtrip_model_json(&initial_model, path, "generated")?;
            save_essence(&initial_model, path, "generated")?;

            if accept {
                std::fs::copy(
                    roundtrip_model_json_path(path, "generated"),
                    roundtrip_model_json_path(path, "expected"),
                )?;
                std::fs::copy(
                    roundtrip_essence_path(path, "generated"),
                    roundtrip_essence_path(path, "expected"),
                )?;
            }

            if !accept && !Path::new(&roundtrip_model_json_path(path, "expected")).exists() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "Expected output file not found: {}",
                        AcceptMode::refresh_hint()
                    ),
                )));
            }

            let expected_model_json = read_roundtrip_model_json_snapshot(path, "expected")?;
            let generated_model_json = read_roundtrip_model_json_snapshot(path, "generated")?;
            assert_eq!(generated_model_json, expected_model_json);

            let expected_essence = fs::read_to_string(roundtrip_essence_path(path, "expected"))?;
            let generated_essence = fs::read_to_string(roundtrip_essence_path(path, "generated"))?;
            assert_eq!(expected_essence, generated_essence);

            let reparsed_model =
                parse(&roundtrip_essence_path(path, "generated"), context.clone())?;
            assert_eq!(generated_essence, format!("{reparsed_model}\n"));

            return Ok(expected_roundtrip_files_for_case(true));
        }

        Err(parse_error) => {
            save_parse_error(&parse_error, path, "generated")?;

            if accept {
                std::fs::copy(
                    roundtrip_error_path(path, "generated"),
                    roundtrip_error_path(path, "expected"),
                )?;
            }

            if !accept && !Path::new(&roundtrip_error_path(path, "expected")).exists() {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "Expected output file not found: {}",
                        AcceptMode::refresh_hint()
                    ),
                )));
            }

            let expected_error = fs::read_to_string(roundtrip_error_path(path, "expected"))?;
            let generated_error = fs::read_to_string(roundtrip_error_path(path, "generated"))?;
            assert_eq!(expected_error, generated_error);

            return Ok(expected_roundtrip_files_for_case(false));
        }
    }
}

/// Returns the roundtrip model JSON path for a parser case and model type.
fn roundtrip_model_json_path(path: &str, file_type: &str) -> String {
    format!("{path}/{file_type}.serialised.json")
}

/// Returns the roundtrip Essence path for a parser case and model type.
fn roundtrip_essence_path(path: &str, file_type: &str) -> String {
    format!("{path}/{file_type}.essence")
}

/// Returns the roundtrip parser-error path for a parser case and model type.
fn roundtrip_error_path(path: &str, file_type: &str) -> String {
    format!("{path}/{file_type}-error.txt")
}

/// Returns the expected snapshot files for a roundtrip parser case outcome.
fn expected_roundtrip_files_for_case(parse_succeeded: bool) -> BTreeSet<String> {
    if parse_succeeded {
        BTreeSet::from([
            format!("expected.serialised.json"),
            format!("expected.essence"),
        ])
    } else {
        BTreeSet::from([format!("expected-error.txt")])
    }
}

/// Serialises and writes a generated model snapshot for roundtrip comparison.
fn save_roundtrip_model_json(
    model: &Model,
    path: &str,

    file_type: &str,
) -> Result<(), std::io::Error> {
    let serialised = serialize_model(model).map_err(std::io::Error::other)?;
    let serialised = truncate_to_first_chars(&serialised, DEFAULT_TEXT_SNAPSHOT_CHARACTER_LIMIT);
    fs::write(
        roundtrip_model_json_path(path, file_type),
        format!("{serialised}\n"),
    )?;
    Ok(())
}

/// Reads a saved roundtrip model JSON snapshot.
fn read_roundtrip_model_json_snapshot(
    path: &str,
    file_type: &str,
) -> Result<String, std::io::Error> {
    let serialised = fs::read_to_string(roundtrip_model_json_path(path, file_type))?;
    Ok(truncate_to_first_chars(
        &serialised,
        DEFAULT_TEXT_SNAPSHOT_CHARACTER_LIMIT,
    ))
}

/// Saves a model as an Essence file.
fn save_essence(model: &Model, path: &str, file_type: &str) -> Result<(), std::io::Error> {
    let filename = roundtrip_essence_path(path, file_type);
    fs::write(filename, format!("{model}\n"))?;
    Ok(())
}

/// Saves a parse error message as a text file.
fn save_parse_error(
    error: &ParseErrorCollection,
    path: &str,
    file_type: &str,
) -> Result<(), std::io::Error> {
    let filename = roundtrip_error_path(path, file_type);
    fs::write(filename, format!("{error}\n"))?;
    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/gen_tests_roundtrip.rs"));

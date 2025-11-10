use conjure_cp::parse::tree_sitter::parse_essence_file;
use conjure_cp::context::Context;
use conjure_cp_cli::utils::testing::{
    read_model_json, serialize_model
};
use conjure_cp::Model;

use std::env;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;
use std::error::Error;
use std::fs::File;
use std::io::Write;

// Designed to test if an Essence feature can be parsed correctly into the AST
// Does not consider rewriting or solving
fn feature_ast_test(path: &str, essence_base: &str, extension: &str) -> Result<(), Box<dyn Error>> {
    /*
    When ACCEPT=true:
        Convert an Essence file into an ast-json using conjure
        Attempt to create a model in conjure-oxide by parsing the ast-json
        Save the model back out as a JSON

    Parse your ground-truth JSON back to being a model
    Output it as a new JSON
    Compare the two
    */
    let accept = env::var("ACCEPT").unwrap_or("false".to_string()) == "true";

    let file_path = format!("{path}/{essence_base}.{extension}");
    let context: Arc<RwLock<Context<'static>>> = Default::default();

    if accept {
        // When accept, always parse a new model because this will be used to remake in the AST
        // Essence is only parsed when ACCEPT=true
        let parsed = parse_essence_file(&file_path, context.clone())?;
        save_parse_model_json(&parsed, path, essence_base, "expected")?;
    }
    // Ensures ACCEPT=true has been run at least once
    if !Path::new(&format!("{path}/{essence_base}.expected-parse.serialised.json")).exists(){
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Expected output file not found: Run with ACCEPT=true"),
        )));
    }
    let model = read_model_json(&context.clone(), path,essence_base,"expected","parse")?;
    save_parse_model_json(&model, path, essence_base, "generated")?;

    // Compare the two models
    let expected_model = read_model_json(&context, path, essence_base, "expected", "parse");
    let model_from_file = read_model_json(&context, path, essence_base, "generated", "parse");
    match (expected_model, model_from_file) {
        (Err(e), _) | (_, Err(e)) => {
            return Err(Box::new(e));
        }
        (Ok(expected_model), Ok(model_from_file)) => {
            assert_eq!(model_from_file, expected_model);
        }
    }

    Ok(())
}

/* Saves the model, but allows for direct saving as "expected",
   as opposed to copying and renaming as done in integration_tests.rs */
fn save_parse_model_json(
    model: &Model,
    path: &str,
    test_name: &str,
    model_type: &str,
) -> Result<(), std::io::Error> {
    let generated_json_str = serialize_model(model)?;
    let filename = format!("{path}/{test_name}.{model_type}-parse.serialised.json");
    File::create(&filename)?.write_all(generated_json_str.as_bytes())?;
    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/gen_tests_feature_AST.rs"));
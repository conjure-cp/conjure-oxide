use conjure_cp::parse::tree_sitter::parse_essence_file;
use conjure_cp::context::Context;
use conjure_cp_cli::utils::testing::{
    read_model_json, save_model_json
};
use conjure_cp::Model;

use std::env;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;
use std::error::Error;
use std::fs;

use std::io::Write;

// Designed to test if an Essence feature can be parsed correctly into the AST and complete a roundtrip
// Does not consider rewriting or solving
fn roundtrip_test(path: &str, filename: &str, extension: &str) -> Result<(), Box<dyn Error>> {
    /*
    Parses Essence file
    Saves generated AST model JSON
    Saves generated Essence

    Compares expected and generated AST model JSON
    Compares expected and generated Essence

    Parses generated Essence back to being a model
    Saves new model as Essence (generated2)
    Compare initally generated Essence with newly generated Essence
    */

    let accept = env::var("ACCEPT").unwrap_or("false".to_string()) == "true";

    let file_path = format!("{path}/{filename}.{extension}");
    let context: Arc<RwLock<Context<'static>>> = Default::default();
    
    let initial_model = parse_essence_file(&file_path, context.clone())?;
    save_model_json(&initial_model, path, filename, "parse")?;
    save_essence(&initial_model, path, filename, "generated")?;

    // When ACCEPT = true, copy over generated to expected
    if accept {
        std::fs::copy(
            format!("{path}/{filename}.generated-parse.serialised.json"),
            format!("{path}/{filename}.expected-parse.serialised.json"),
        )?;
        std::fs::copy(
            format!("{path}/{filename}.generated-essence.essence"),
            format!("{path}/{filename}.expected-essence.essence"),
        )?;
    }

    // Ensures ACCEPT=true has been run at least once
    if !Path::new(&format!("{path}/{filename}.expected-parse.serialised.json")).exists(){
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Expected output file not found: Run with ACCEPT=true"),
        )));
    }

    // Compare the expected and generated model
    let expected_model = read_model_json(&context, path, filename, "expected", "parse")?;
    let generated_model = read_model_json(&context, path, filename, "generated", "parse")?;
    assert_eq!(generated_model, expected_model);

    // Compares essence files
    let expected_essence = fs::read_to_string(&format!("{path}/{filename}.expected-essence.essence"))?;
    let generated_essence = fs::read_to_string(&format!("{path}/{filename}.generated-essence.essence"))?;
    assert_eq!(expected_essence,generated_essence);

    // Compares roundtrip
    let new_model = parse_essence_file(&format!("{path}/{filename}.generated-essence.essence"), context.clone())?;
    save_essence(&new_model, path, filename, "generated2")?;
    let new_generated_essence = fs::read_to_string(&format!("{path}/{filename}.generated2-essence.essence"))?;
    assert_eq!(generated_essence,new_generated_essence);

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
    write!(file,"{}",model)?;
    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/gen_tests_roundtrip.rs"));
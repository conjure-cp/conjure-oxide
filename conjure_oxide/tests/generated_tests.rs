use std::env;
use std::error::Error;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

use conjure_core::context::Context;
use conjure_oxide::rule_engine::resolve_rule_sets;
use conjure_oxide::rule_engine::rewrite_model;
use conjure_oxide::utils::conjure::{get_minion_solutions, parse_essence_file};
use conjure_oxide::utils::testing::save_stats_json;
use conjure_oxide::utils::testing::{
    read_minion_solutions_json, read_model_json, save_minion_solutions_json, save_model_json,
};
use conjure_oxide::SolverFamily;

fn main() {
    let file_path = Path::new("/path/to/your/file.txt");
    let base_name = file_path.file_stem().and_then(|stem| stem.to_str());

    match base_name {
        Some(name) => println!("Base name: {}", name),
        None => println!("Could not extract the base name"),
    }
}

pub fn integration_test(path: &str, essence_base: &str) -> Result<(), Box<dyn Error>> {
    // --------------------------------------------------------------------------------
    // -- parsing the essence file

    // calling conjure to convert Essence to astjson
    let mut cmd = std::process::Command::new("conjure");
    let output = cmd
        .arg("pretty")
        .arg("--output-format=astjson")
        .arg(format!("{path}/{essence_base}.essence"))
        .output()?;
    let stderr_string = String::from_utf8(output.stderr)?;
    assert!(
        stderr_string.is_empty(),
        "conjure's stderr is not empty: {}",
        stderr_string
    );
    let astjson = String::from_utf8(output.stdout)?;

    // "parsing" astjson as Model
    let parsed_model = model_from_json(&astjson)?;

    // a consistent sorting of the keys of json objects
    // only required for the generated version
    // since the expected version will already be sorted
    let generated_json = sort_json_object(&serde_json::to_value(parsed_model.clone())?);

    // serialise to file
    let generated_json_str = serde_json::to_string_pretty(&generated_json)?;
    File::create(format!(
        "{path}/{essence_base}.generated-parse.serialised.json"
    ))?
    .write_all(generated_json_str.as_bytes())?;

    if std::env::var("ACCEPT").map_or(false, |v| v == "true") {
        std::fs::copy(
            format!("{path}/{essence_base}.generated-parse.serialised.json"),
            format!("{path}/{essence_base}.expected-parse.serialised.json"),
        )?;
    }

    // --------------------------------------------------------------------------------
    // -- reading the expected version from the filesystem

    let expected_parse_str = std::fs::read_to_string(format!(
        "{path}/{essence_base}.expected-parse.serialised.json"
    ))?;

    let expected_parse_mdl: Model = serde_json::from_str(&expected_parse_str)?;

    // --------------------------------------------------------------------------------
    // assert that they are the same model

    assert_eq!(model, expected_model);

    // --------------------------------------------------------------------------------

    // ToDo: There is A LOT of duplication here. We should refactor this to a function. In my defense, it was midnight when I wrote this.

    let rewritten_model = rewrite_model(&parsed_model);

    let rewritten_json = sort_json_object(&serde_json::to_value(rewritten_model.clone())?);
    let rewritten_json_str = serde_json::to_string_pretty(&rewritten_json)?;
    File::create(format!(
        "{path}/{essence_base}.generated-rewrite.serialised.json"
    ))?
    .write_all(rewritten_json_str.as_bytes())?;

    if std::env::var("ACCEPT").map_or(false, |v| v == "true") {
        std::fs::copy(
            format!("{path}/{essence_base}.generated-rewrite.serialised.json"),
            format!("{path}/{essence_base}.expected-rewrite.serialised.json"),
        )?;
    }

    // --------------------------------------------------------------------------------
    // -- reading the expected version from the filesystem

    let expected_rewrite_str = std::fs::read_to_string(format!(
        "{path}/{essence_base}.expected-rewrite.serialised.json"
    ))?;

    let expected_rewrite_mdl: Model = serde_json::from_str(&expected_rewrite_str)?;

    // --------------------------------------------------------------------------------
    // assert that they are the same model

    assert_eq!(rewritten_model, expected_rewrite_mdl);

    Ok(())
}

/// Recursively sorts the keys of all JSON objects within the provided JSON value.
///
/// serde_json will output JSON objects in an arbitrary key order.
/// this is normally fine, except in our use case we wouldn't want to update the expected output again and again.
/// so a consistent (sorted) ordering of the keys is desirable.
fn sort_json_object(value: &Value) -> Value {
    match value {
        Value::Object(obj) => {
            let mut ordered: Vec<(String, Value)> = obj
                .iter()
                .map(|(k, v)| {
                    if k == "variables" {
                        (k.clone(), sort_json_variables(v))
                    } else {
                        (k.clone(), sort_json_object(v))
                    }
                })
                // .map(|(k, v)| (k.clone(), sort_json_object(v)))
                .collect();
            ordered.sort_by(|a, b| a.0.cmp(&b.0));

            Value::Object(ordered.into_iter().collect())
        }
        Value::Array(arr) => Value::Array(arr.iter().map(sort_json_object).collect()),
        _ => value.clone(),
    }
}

/// Sort the "variables" field by name.
/// We have to do this separately becasue that field is not a JSON object, instead it's an array of tuples.
fn sort_json_variables(value: &Value) -> Value {
    match value {
        Value::Array(vars) => {
            let mut vars_sorted = vars.clone();
            vars_sorted.sort_by(|a, b| {
                let a_obj = &a.as_array().unwrap()[0];
                let a_name: conjure_oxide::ast::Name =
                    serde_json::from_value(a_obj.clone()).unwrap();

                let b_obj = &b.as_array().unwrap()[0];
                let b_name: conjure_oxide::ast::Name =
                    serde_json::from_value(b_obj.clone()).unwrap();

                a_name.cmp(&b_name)
            });
            Value::Array(vars_sorted)
        }
        _ => value.clone(),
    }
}

#[test]
fn assert_conjure_present() {
    conjure_oxide::find_conjure::conjure_executable().unwrap();
}

include!(concat!(env!("OUT_DIR"), "/gen_tests.rs"));

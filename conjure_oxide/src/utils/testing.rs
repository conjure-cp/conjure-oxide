use crate::rule_engine::resolve_rules::resolve_rule_sets;
use crate::rule_engine::rewrite::rewrite_model;
use crate::utils::json::sort_json_object;
use crate::utils::misc::to_set;
use conjure_core::ast::Model;
use conjure_rules::RuleSet;
use serde_json::Error as JsonError;
use std::collections::HashSet;
use std::fmt::Debug;
use std::fs::File;
use std::hash::Hash;
use std::io::Write;
use std::process::exit;

pub fn assert_eq_any_order<T: Eq + Hash + Debug + Clone>(a: &Vec<Vec<T>>, b: &Vec<Vec<T>>) {
    assert_eq!(a.len(), b.len());

    let mut a_rows: Vec<HashSet<T>> = Vec::new();
    for row in a {
        let hash_row = to_set(row);
        a_rows.push(hash_row);
    }

    let mut b_rows: Vec<HashSet<T>> = Vec::new();
    for row in b {
        let hash_row = to_set(row);
        b_rows.push(hash_row);
    }

    println!("{:?},{:?}", a_rows, b_rows);
    for row in a_rows {
        assert!(b_rows.contains(&row));
    }
}

pub fn serialise_model(model: &Model) -> Result<String, JsonError> {
    // A consistent sorting of the keys of json objects
    // only required for the generated version
    // since the expected version will already be sorted
    let generated_json = sort_json_object(&serde_json::to_value(model.clone())?);

    // serialise to string
    let generated_json_str = serde_json::to_string_pretty(&generated_json)?;

    Ok(generated_json_str)
}

pub fn save_model_json(
    model: &Model,
    path: &str,
    test_name: &str,
    test_stage: &str,
    accept: bool,
) -> Result<(), std::io::Error> {
    let generated_json_str = serialise_model(model)?;

    File::create(format!(
        "{path}/{test_name}.generated-{test_stage}.serialised.json"
    ))?
    .write_all(generated_json_str.as_bytes())?;

    if accept {
        std::fs::copy(
            format!("{path}/{test_name}.generated-{test_stage}.serialised.json"),
            format!("{path}/{test_name}.expected-{test_stage}.serialised.json"),
        )?;
    }

    Ok(())
}

pub fn read_model_json(
    path: &str,
    test_name: &str,
    test_stage: &str,
) -> Result<Model, std::io::Error> {
    let expected_json_str = std::fs::read_to_string(format!(
        "{path}/{test_name}.expected-{test_stage}.serialised.json"
    ))?;

    let expected_model: Model = serde_json::from_str(&expected_json_str)?;

    Ok(expected_model)
}

pub fn resolve_rule_sets_or_panic(rs_names: Vec<&str>) -> Vec<&'static RuleSet<'static>> {
    match resolve_rule_sets(rs_names) {
        Ok(rs) => rs,
        Err(e) => {
            panic!("Error resolving rule sets: {}", e);
        }
    }
}

pub fn rewrite_or_panic(model: &Model, rule_sets: &Vec<&'static RuleSet<'static>>) -> Model {
    match rewrite_model(model, rule_sets) {
        Ok(model) => model,
        Err(e) => {
            panic!("Error rewriting model: {:?}", e);
        }
    }
}

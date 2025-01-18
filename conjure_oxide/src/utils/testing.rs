use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Debug;

use std::fs::File;
use std::fs::{read_to_string, OpenOptions};
use std::hash::Hash;
use std::io::Write;
use std::sync::{Arc, RwLock};

use conjure_core::context::Context;
use serde_json::{json, Error as JsonError, Value as JsonValue};

use conjure_core::error::Error;

use crate::ast::Name::UserName;
use crate::ast::{Literal, Name};
use crate::utils::conjure::minion_solutions_to_json;
use crate::utils::json::sort_json_object;
use crate::utils::misc::to_set;
use crate::Model as ConjureModel;

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

pub fn serialise_model(model: &ConjureModel) -> Result<String, JsonError> {
    // A consistent sorting of the keys of json objects
    // only required for the generated version
    // since the expected version will already be sorted
    let generated_json = sort_json_object(&serde_json::to_value(model.clone())?, false);

    // serialise to string
    let generated_json_str = serde_json::to_string_pretty(&generated_json)?;

    Ok(generated_json_str)
}

pub fn save_model_json(
    model: &ConjureModel,
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

pub fn save_stats_json(
    context: Arc<RwLock<Context<'static>>>,
    path: &str,
    test_name: &str,
) -> Result<(), std::io::Error> {
    #[allow(clippy::unwrap_used)]
    let stats = context.read().unwrap().clone();
    let generated_json = sort_json_object(&serde_json::to_value(stats)?, false);

    // serialise to string
    let generated_json_str = serde_json::to_string_pretty(&generated_json)?;

    File::create(format!("{path}/{test_name}-stats.json"))?
        .write_all(generated_json_str.as_bytes())?;

    Ok(())
}

pub fn read_model_json(
    path: &str,
    test_name: &str,
    prefix: &str,
    test_stage: &str,
) -> Result<ConjureModel, std::io::Error> {
    let expected_json_str = std::fs::read_to_string(format!(
        "{path}/{test_name}.{prefix}-{test_stage}.serialised.json"
    ))?;

    let expected_model: ConjureModel = serde_json::from_str(&expected_json_str)?;

    Ok(expected_model)
}

pub fn minion_solutions_from_json(
    serialized: &str,
) -> Result<Vec<HashMap<Name, Literal>>, anyhow::Error> {
    let json: JsonValue = serde_json::from_str(serialized)?;

    let json_array = json
        .as_array()
        .ok_or(Error::Parse("Invalid JSON".to_owned()))?;

    let mut solutions = Vec::new();

    for solution in json_array {
        let mut sol = HashMap::new();
        let solution = solution
            .as_object()
            .ok_or(Error::Parse("Invalid JSON".to_owned()))?;

        for (var_name, constant) in solution {
            let constant = match constant {
                JsonValue::Number(n) => {
                    let n = n
                        .as_i64()
                        .ok_or(Error::Parse("Invalid integer".to_owned()))?;
                    Literal::Int(n as i32)
                }
                JsonValue::Bool(b) => Literal::Bool(*b),
                _ => return Err(Error::Parse("Invalid constant".to_owned()).into()),
            };

            sol.insert(UserName(var_name.into()), constant);
        }

        solutions.push(sol);
    }

    Ok(solutions)
}

pub fn save_minion_solutions_json(
    solutions: &Vec<BTreeMap<Name, Literal>>,
    path: &str,
    test_name: &str,
    accept: bool,
) -> Result<JsonValue, std::io::Error> {
    let json_solutions = minion_solutions_to_json(solutions);

    let generated_json_str = serde_json::to_string_pretty(&json_solutions)?;

    File::create(format!(
        "{path}/{test_name}.generated-minion.solutions.json"
    ))?
    .write_all(generated_json_str.as_bytes())?;

    if accept {
        std::fs::copy(
            format!("{path}/{test_name}.generated-minion.solutions.json"),
            format!("{path}/{test_name}.expected-minion.solutions.json"),
        )?;
    }

    Ok(json_solutions)
}

pub fn read_minion_solutions_json(
    path: &str,
    test_name: &str,
    prefix: &str,
) -> Result<JsonValue, anyhow::Error> {
    let expected_json_str =
        std::fs::read_to_string(format!("{path}/{test_name}.{prefix}-minion.solutions.json"))?;

    let expected_solutions: JsonValue =
        sort_json_object(&serde_json::from_str(&expected_json_str)?, true);

    Ok(expected_solutions)
}

pub fn read_rule_trace(
    path: &str,
    test_name: &str,
    prefix: &str,
    accept: bool,
) -> Result<JsonValue, anyhow::Error> {
    let filename = format!("{path}/{test_name}-{prefix}-rule-trace.json");

    let rule_traces = if prefix == "generated" {
        count_and_sort_rules(&filename)?
    } else {
        let file_contents = std::fs::read_to_string(filename)?;
        serde_json::from_str(&file_contents)?
    };

    // if accept {
    std::fs::copy(
        format!("{path}/{test_name}-generated-rule-trace.json"),
        format!("{path}/{test_name}-expected-rule-trace.json"),
    )?;
    // }

    Ok(rule_traces)
}

pub fn count_and_sort_rules(filename: &str) -> Result<JsonValue, anyhow::Error> {
    let file_contents = read_to_string(filename)?;

    let sorted_json_rules = if file_contents.trim().is_empty() {
        let rule_count_message = json!({
            "Number of rules applied": 0,
        });
        rule_count_message
    } else {
        let rule_count = file_contents.lines().count();
        let mut sorted_json_rules = sort_json_rules(&file_contents)?;

        let rule_count_message = json!({
            "Number of rules applied": rule_count,
        });

        if let Some(array) = sorted_json_rules.as_array_mut() {
            array.push(rule_count_message);
        } else {
            return Err(anyhow::anyhow!("Expected JSON array"));
        }
        sort_json_object(&sorted_json_rules, false)
    };

    let generated_sorted_json_rules = serde_json::to_string_pretty(&sorted_json_rules)?;

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(filename)?;

    file.write_all(generated_sorted_json_rules.as_bytes())?;

    Ok(sorted_json_rules)
}

fn sort_json_rules(json_rule_traces: &str) -> Result<JsonValue, anyhow::Error> {
    let mut sorted_rule_traces = Vec::new();

    for line in json_rule_traces.lines() {
        let pretty_json = sort_json_object(&serde_json::from_str(line)?, true);
        sorted_rule_traces.push(pretty_json);
    }

    Ok(JsonValue::Array(sorted_rule_traces))
}

pub fn read_human_rule_trace(
    path: &str,
    test_name: &str,
    prefix: &str,
    accept: bool,
) -> Result<Vec<String>, std::io::Error> {
    let filename = format!("{path}/{test_name}-{prefix}-rule-trace-human.txt");
    let rules_trace: Vec<String> = read_to_string(&filename)
        .unwrap()
        .lines()
        .map(String::from)
        .collect();

    if accept {
        std::fs::copy(
            format!("{path}/{test_name}-generated-rule-trace-human.txt"),
            format!("{path}/{test_name}-expected-rule-trace-human.txt"),
        )?;
    }

    Ok(rules_trace)
}

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Debug;
use std::{io, vec};

use conjure_cp::ast::records::RecordValue;
use conjure_cp::bug;
use itertools::Itertools as _;
use std::fs::File;
use std::fs::OpenOptions;
use std::hash::Hash;
use std::io::Write;
use std::sync::{Arc, RwLock};
use uniplate::Uniplate;

use conjure_cp::ast::{AbstractLiteral, Domain, SerdeModel};
use conjure_cp::context::Context;
use serde_json::{Error as JsonError, Value as JsonValue, json};

use conjure_cp::error::Error;

use crate::utils::conjure::solutions_to_json;
use crate::utils::json::sort_json_object;
use crate::utils::misc::to_set;
use conjure_cp::Model as ConjureModel;
use conjure_cp::ast::Name::User;
use conjure_cp::ast::{Literal, Name};
use conjure_cp::solver::SolverFamily;

/// Converts a SerdeModel to JSON with stable IDs.
///
/// This ensures that the same model structure always produces the same IDs,
/// regardless of the order in which objects were created in memory.
fn model_tojson_with_stable_ids(model: &SerdeModel) -> Result<JsonValue, JsonError> {
    // Collect stable ID mapping using uniplate traversal on the SerdeModel
    let id_map_u32 = model.collect_stable_id_mapping();

    // Convert to u64 for JSON processing (serde_json::Number only has as_u64())
    let id_map: HashMap<u64, u64> = id_map_u32
        .into_iter()
        .map(|(k, v)| (k as u64, v as u64))
        .collect();

    // Serialize the model to JSON
    let mut json = serde_json::to_value(model)?;

    // Replace all IDs in the JSON with their stable counterparts
    replace_ids(&mut json, &id_map);

    Ok(json)
}

/// Recursively replaces all IDs in the JSON with their stable counterparts.
///
/// This is applied to all fields that are called "id" or "ptr" - be mindful
/// of potential naming clashes in the future!
fn replace_ids(value: &mut JsonValue, id_map: &HashMap<u64, u64>) {
    match value {
        JsonValue::Object(map) => {
            // Replace IDs in three places:
            // - "id" fields (SymbolTable IDs)
            // - "parent" fields (SymbolTable nesting)
            // - "ptr" fields (DeclarationPtr IDs)
            for (k, v) in map.iter_mut() {
                if (k == "id" || k == "ptr" || k == "parent")
                    && let JsonValue::Number(n) = v
                    && let Some(id) = n.as_u64()
                    && let Some(&stable_id) = id_map.get(&id)
                {
                    *v = JsonValue::Number(stable_id.into());
                }
            }

            // Recursively process all values
            for val in map.values_mut() {
                replace_ids(val, id_map);
            }
        }
        JsonValue::Array(arr) => {
            for item in arr {
                replace_ids(item, id_map);
            }
        }
        _ => {}
    }
}

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

    println!("{a_rows:?},{b_rows:?}");
    for row in a_rows {
        assert!(b_rows.contains(&row));
    }
}

pub fn serialize_model(model: &ConjureModel) -> Result<String, JsonError> {
    let serde_model: SerdeModel = model.clone().into();

    // Convert to JSON with stable IDs
    let json_with_stable_ids = model_tojson_with_stable_ids(&serde_model)?;

    // Sort JSON object keys for consistent output
    let sorted_json = sort_json_object(&json_with_stable_ids, false);

    // Serialize to pretty-printed string
    serde_json::to_string_pretty(&sorted_json)
}

pub fn save_model_json(
    model: &ConjureModel,
    path: &str,
    test_name: &str,
    test_stage: &str,
) -> Result<(), std::io::Error> {
    let generated_json_str = serialize_model(model)?;
    let filename = format!("{path}/{test_name}.generated-{test_stage}.serialised.json");
    File::create(&filename)?.write_all(generated_json_str.as_bytes())?;
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

/// Reads a file into a `String`, providing a clearer error message that includes the file path.
fn read_with_path(path: String) -> Result<String, std::io::Error> {
    std::fs::read_to_string(&path)
        .map_err(|e| io::Error::new(e.kind(), format!("{} (path: {})", e, path)))
}

pub fn read_model_json(
    ctx: &Arc<RwLock<Context<'static>>>,
    path: &str,
    test_name: &str,
    prefix: &str,
    test_stage: &str,
) -> Result<ConjureModel, std::io::Error> {
    let expected_json_str = read_with_path(format!(
        "{path}/{test_name}.{prefix}-{test_stage}.serialised.json"
    ))?;
    println!("{path}/{test_name}.{prefix}-{test_stage}.serialised.json");
    let expected_model: SerdeModel = serde_json::from_str(&expected_json_str)?;

    Ok(expected_model.initialise(ctx.clone()).unwrap())
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

            sol.insert(User(var_name.into()), constant);
        }

        solutions.push(sol);
    }

    Ok(solutions)
}

/// Writes the minion solutions to a generated JSON file, and returns the JSON structure.
pub fn save_solutions_json(
    solutions: &Vec<BTreeMap<Name, Literal>>,
    path: &str,
    test_name: &str,
    solver: SolverFamily,
) -> Result<JsonValue, std::io::Error> {
    let json_solutions = solutions_to_json(solutions);
    let generated_json_str = serde_json::to_string_pretty(&json_solutions)?;

    let solver_name = match solver {
        SolverFamily::Sat => "sat",
        SolverFamily::Smt => "smt",
        SolverFamily::Minion => "minion",
    };

    let filename = format!("{path}/{test_name}.generated-{solver_name}.solutions.json");
    File::create(&filename)?.write_all(generated_json_str.as_bytes())?;

    Ok(json_solutions)
}

pub fn read_solutions_json(
    path: &str,
    test_name: &str,
    prefix: &str,
    solver: SolverFamily,
) -> Result<JsonValue, anyhow::Error> {
    let solver_name = match solver {
        SolverFamily::Sat => "sat",
        SolverFamily::Smt => "smt",
        SolverFamily::Minion => "minion",
    };

    let expected_json_str = read_with_path(format!(
        "{path}/{test_name}.{prefix}-{solver_name}.solutions.json"
    ))?;

    let expected_solutions: JsonValue =
        sort_json_object(&serde_json::from_str(&expected_json_str)?, true);

    Ok(expected_solutions)
}

/// Reads a rule trace from a file. For the generated prefix, it appends a count message.
/// Returns the lines of the file as a vector of strings.
pub fn read_rule_trace(
    path: &str,
    test_name: &str,
    prefix: &str,
) -> Result<Vec<String>, std::io::Error> {
    let filename = format!("{path}/{test_name}-{prefix}-rule-trace.json");
    let mut rules_trace: Vec<String> = read_with_path(filename.clone())?
        .lines()
        .map(String::from)
        .collect();

    // If prefix is "generated", append the count message
    if prefix == "generated" {
        let rule_count = rules_trace.len();
        let count_message = json!({
            "message": "Number of rules applied",
            "count": rule_count
        });
        let count_message_string = serde_json::to_string(&count_message)?;
        rules_trace.push(count_message_string);

        // Overwrite the file with updated content (including the count message)
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(filename)?;
        writeln!(file, "{}", rules_trace.join("\n"))?;
    }

    Ok(rules_trace)
}

/// Reads a human-readable rule trace text file.
pub fn read_human_rule_trace(
    path: &str,
    test_name: &str,
    prefix: &str,
) -> Result<Vec<String>, std::io::Error> {
    let filename = format!("{path}/{test_name}-{prefix}-rule-trace-human.txt");
    let rules_trace: Vec<String> = read_with_path(filename)?
        .lines()
        .map(String::from)
        .collect();

    Ok(rules_trace)
}

#[doc(hidden)]
pub fn normalize_solutions_for_comparison(
    input_solutions: &[BTreeMap<Name, Literal>],
) -> Vec<BTreeMap<Name, Literal>> {
    let mut normalized = input_solutions.to_vec();

    for solset in &mut normalized {
        // remove machine names
        let keys_to_remove: Vec<Name> = solset
            .keys()
            .filter(|k| matches!(k, Name::Machine(_)))
            .cloned()
            .collect();
        for k in keys_to_remove {
            solset.remove(&k);
        }

        let mut updates = vec![];
        for (k, v) in solset.clone() {
            if let Name::User(_) = k {
                match v {
                    Literal::Bool(true) => updates.push((k, Literal::Int(1))),
                    Literal::Bool(false) => updates.push((k, Literal::Int(0))),
                    Literal::Int(_) => {}
                    Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, _)) => {
                        // make all domains the same (this is just in the tester so the types dont
                        // actually matter)

                        let mut matrix =
                            AbstractLiteral::Matrix(elems, Box::new(Domain::Int(vec![])));
                        matrix = matrix.transform(&move |x: AbstractLiteral<Literal>| match x {
                            AbstractLiteral::Matrix(items, _) => {
                                let items = items
                                    .into_iter()
                                    .map(|x| match x {
                                        Literal::Bool(false) => Literal::Int(0),
                                        Literal::Bool(true) => Literal::Int(1),
                                        x => x,
                                    })
                                    .collect_vec();

                                AbstractLiteral::Matrix(items, Box::new(Domain::Int(vec![])))
                            }
                            x => x,
                        });
                        updates.push((k, Literal::AbstractLiteral(matrix)));
                    }
                    Literal::AbstractLiteral(AbstractLiteral::Tuple(elems)) => {
                        // just the same as matrix but with tuples instead
                        // only conversion needed is to convert bools to ints
                        let mut tuple = AbstractLiteral::Tuple(elems);
                        tuple = tuple.transform(
                            &(move |x: AbstractLiteral<Literal>| match x {
                                AbstractLiteral::Tuple(items) => {
                                    let items = items
                                        .into_iter()
                                        .map(|x| match x {
                                            Literal::Bool(false) => Literal::Int(0),
                                            Literal::Bool(true) => Literal::Int(1),
                                            x => x,
                                        })
                                        .collect_vec();

                                    AbstractLiteral::Tuple(items)
                                }
                                x => x,
                            }),
                        );
                        updates.push((k, Literal::AbstractLiteral(tuple)));
                    }
                    Literal::AbstractLiteral(AbstractLiteral::Record(entries)) => {
                        // just the same as matrix but with tuples instead
                        // only conversion needed is to convert bools to ints
                        let mut record = AbstractLiteral::Record(entries);
                        record = record.transform(&move |x: AbstractLiteral<Literal>| match x {
                            AbstractLiteral::Record(entries) => {
                                let entries = entries
                                    .into_iter()
                                    .map(|x| {
                                        let RecordValue { name, value } = x;
                                        {
                                            let value = match value {
                                                Literal::Bool(false) => Literal::Int(0),
                                                Literal::Bool(true) => Literal::Int(1),
                                                x => x,
                                            };
                                            RecordValue { name, value }
                                        }
                                    })
                                    .collect_vec();

                                AbstractLiteral::Record(entries)
                            }
                            x => x,
                        });
                        updates.push((k, Literal::AbstractLiteral(record)));
                    }
                    e => bug!("unexpected literal type: {e:?}"),
                }
            }
        }

        for (k, v) in updates {
            solset.insert(k, v);
        }
    }

    // Remove duplicates
    normalized = normalized.into_iter().unique().collect();
    normalized
}

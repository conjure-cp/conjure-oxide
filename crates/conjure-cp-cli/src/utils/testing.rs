use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Debug;
use std::path::Path;
use std::{io, mem, vec};

use conjure_cp::ast::records::Field;
use conjure_cp::ast::serde::ObjId;
use itertools::Itertools as _;
use std::fs::File;
use std::hash::Hash;
use std::io::{BufRead, BufReader};
use std::sync::{Arc, RwLock};
use uniplate::Uniplate;

use conjure_cp::ast::pretty::pretty_expression_domain_annotation;
use conjure_cp::ast::{AbstractLiteral, Expression, GroundDomain, Moo, SerdeModel};
use conjure_cp::context::Context;
use serde_json::{Error as JsonError, Value as JsonValue};

use conjure_cp::error::Error;

use crate::utils::conjure::solutions_to_essence;
use crate::utils::json::sort_json_object;
use crate::utils::misc::to_set;
use conjure_cp::Model as ConjureModel;
use conjure_cp::ast::Name::User;
use conjure_cp::ast::{Literal, Name};
use conjure_cp::settings::SolverFamily;

/// Limit how many lines of the rewrite serialisation we persist/compare in integration tests.
pub const REWRITE_SERIALISED_JSON_MAX_LINES: usize = 1000;

/// Limit how many characters we persist/compare for large text snapshots.
pub const DEFAULT_TEXT_SNAPSHOT_CHARACTER_LIMIT: usize = 1_000_000;

/// Converts a SerdeModel to JSON with stable IDs.
///
/// This ensures that the same model structure always produces the same IDs,
/// regardless of the order in which objects were created in memory.
fn model_to_json_with_stable_ids(model: &SerdeModel) -> Result<JsonValue, JsonError> {
    // Collect stable ID mapping using uniplate traversal on the SerdeModel
    let id_map = model.collect_stable_id_mapping();

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
fn replace_ids(value: &mut JsonValue, id_map: &HashMap<ObjId, ObjId>) {
    match value {
        JsonValue::Object(map) => {
            // Replace IDs in three places:
            // - "id" fields (SymbolTable IDs)
            // - "parent" fields (SymbolTable nesting)
            // - "ptr" fields (DeclarationPtr IDs)
            for (k, v) in map.iter_mut() {
                if (k == "id" || k == "ptr" || k == "parent")
                    && let Ok(old_id) = serde_json::from_value::<ObjId>(mem::take(v))
                {
                    let new_id = id_map.get(&old_id).expect("all ids to be in the id map");
                    *v = serde_json::to_value(new_id)
                        .expect("serialization of an ObjId to always succeed");
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

    for row in a_rows {
        assert!(b_rows.contains(&row));
    }
}

pub fn serialize_model(model: &ConjureModel) -> Result<String, JsonError> {
    let serde_model: SerdeModel = model.clone().into();

    // Convert to JSON with stable IDs
    let json_with_stable_ids = model_to_json_with_stable_ids(&serde_model)?;

    // Sort JSON object keys for consistent output
    let sorted_json = sort_json_object(&json_with_stable_ids, false);

    // Serialize to pretty-printed string
    serde_json::to_string_pretty(&sorted_json)
}

pub fn serialize_domains(model: &ConjureModel) -> Result<String, JsonError> {
    let mut output = String::new();
    for constraint in model.constraints() {
        serialize_domains_expr(constraint, 0, &mut output);
    }
    Ok(output)
}

fn serialize_domains_expr(expr: &Expression, depth: usize, output: &mut String) {
    let domain = expr
        .domain_of()
        .map(|domain| domain.to_string())
        .unwrap_or_else(|| "<unknown>".to_owned());
    output.push_str(&" ".repeat(depth));
    output.push_str(&pretty_expression_domain_annotation(expr, domain));
    output.push('\n');

    for child in expr.children() {
        serialize_domains_expr(&child, depth + 1, output);
    }
}

pub fn save_model_json(
    model: &ConjureModel,
    path: &str,
    test_name: &str,
    test_stage: &str,
    solver: SolverFamily,
) -> Result<(), std::io::Error> {
    let marker = solver.as_str();
    let generated_json_str = serialize_model(model)?;
    let generated_json_str = maybe_truncate_serialised_json(generated_json_str, test_stage);
    let filename = format!("{path}/{test_name}-{marker}.generated-{test_stage}.serialised.json");
    println!("saving: {filename}");
    std::fs::write(filename, format!("{generated_json_str}\n"))?;
    Ok(())
}

pub fn save_stats_json(
    context: Arc<RwLock<Context<'static>>>,
    path: &str,
    test_name: &str,
    solver: SolverFamily,
) -> Result<(), std::io::Error> {
    #[allow(clippy::unwrap_used)]
    let solver_name = solver.as_str();

    let stats = context.read().unwrap().clone();
    let generated_json = sort_json_object(&serde_json::to_value(stats)?, false);

    // serialise to string
    let generated_json_str = serde_json::to_string_pretty(&generated_json)?;

    std::fs::write(
        format!("{path}/{test_name}-{solver_name}-stats.json"),
        format!("{generated_json_str}\n"),
    )?;

    Ok(())
}

/// Reads a file into a `String`, providing a clearer error message that includes the file path.
fn read_with_path(path: String) -> Result<String, std::io::Error> {
    std::fs::read_to_string(&path)
        .map_err(|e| io::Error::new(e.kind(), format!("{e} (path: {path})")))
}

pub fn read_model_json(
    ctx: &Arc<RwLock<Context<'static>>>,
    path: &str,
    test_name: &str,
    prefix: &str,
    test_stage: &str,
    solver: SolverFamily,
) -> Result<ConjureModel, std::io::Error> {
    let marker = solver.as_str();
    let filepath = format!("{path}/{test_name}-{marker}.{prefix}-{test_stage}.serialised.json");
    let expected_json_str = std::fs::read_to_string(filepath)?;
    let expected_model: SerdeModel = serde_json::from_str(&expected_json_str)?;

    Ok(expected_model.initialise(ctx.clone()).unwrap())
}

/// Reads only the first `max_lines` from a serialised model JSON file.
pub fn read_model_json_prefix(
    path: &str,
    test_name: &str,
    prefix: &str,
    test_stage: &str,
    solver: SolverFamily,
    max_lines: usize,
) -> Result<String, std::io::Error> {
    let marker = solver.as_str();
    let filename = format!("{path}/{test_name}-{marker}.{prefix}-{test_stage}.serialised.json");
    println!("reading: {filename}");
    read_first_n_lines(filename, max_lines)
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

/// Writes solutions in Conjure's multi-solution Essence format.
pub fn save_solutions_essence(
    solutions: &[BTreeMap<Name, Literal>],
    path: &str,
    test_name: &str,
    solver: SolverFamily,
) -> Result<String, std::io::Error> {
    let rendered = solutions_to_essence(solutions);
    let solver_name = solver.as_str();
    let filename = format!("{path}/{test_name}-{solver_name}.generated.solutions");
    std::fs::write(filename, &rendered)?;

    Ok(rendered)
}

pub fn read_solutions_essence(
    path: &str,
    test_name: &str,
    prefix: &str,
    solver: SolverFamily,
) -> Result<String, anyhow::Error> {
    let solver_name = solver.as_str();
    let filename = format!("{path}/{test_name}-{solver_name}.{prefix}.solutions");
    Ok(read_with_path(filename)?)
}

/// Reads a default rule trace text file.
pub fn read_default_rule_trace(
    path: &str,
    test_name: &str,
    prefix: &str,
    solver: &SolverFamily,
) -> Result<String, std::io::Error> {
    let solver_name = solver.as_str();
    let filename = format!("{path}/{test_name}-{solver_name}-{prefix}-rule-trace.txt");
    Ok(truncate_to_first_chars(
        &read_with_path(filename)?,
        DEFAULT_TEXT_SNAPSHOT_CHARACTER_LIMIT,
    ))
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
                            AbstractLiteral::Matrix(elems, Moo::new(GroundDomain::Int(vec![])));
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

                                AbstractLiteral::Matrix(items, Moo::new(GroundDomain::Int(vec![])))
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
                                        let Field { name, value } = x;
                                        {
                                            let value = match value {
                                                Literal::Bool(false) => Literal::Int(0),
                                                Literal::Bool(true) => Literal::Int(1),
                                                x => x,
                                            };
                                            Field { name, value }
                                        }
                                    })
                                    .collect_vec();

                                AbstractLiteral::Record(entries)
                            }
                            x => x,
                        });
                        updates.push((k, Literal::AbstractLiteral(record)));
                    }
                    Literal::AbstractLiteral(AbstractLiteral::Variant(entry)) => {
                        let mut variant = AbstractLiteral::Variant(entry);
                        variant = variant.transform(&move |x| match x {
                            AbstractLiteral::Variant(entry) => {
                                let Field { name, value } = Moo::unwrap_or_clone(entry);
                                let value = match value {
                                    Literal::Bool(false) => Literal::Int(0),
                                    Literal::Bool(true) => Literal::Int(1),
                                    value => value,
                                };
                                AbstractLiteral::Variant(Moo::new(Field { name, value }))
                            }
                            value => value,
                        });
                        updates.push((k, Literal::AbstractLiteral(variant)));
                    }
                    Literal::AbstractLiteral(AbstractLiteral::Set(members)) => {
                        let set = AbstractLiteral::Set(members).transform(&move |x| match x {
                            AbstractLiteral::Set(members) => {
                                let members = members
                                    .into_iter()
                                    .map(|x| match x {
                                        Literal::Bool(false) => Literal::Int(0),
                                        Literal::Bool(true) => Literal::Int(1),
                                        x => x,
                                    })
                                    .collect_vec();

                                AbstractLiteral::Set(members)
                            }
                            x => x,
                        });
                        updates.push((k, Literal::AbstractLiteral(set)));
                    }
                    Literal::AbstractLiteral(AbstractLiteral::MSet(members)) => {
                        let mset = AbstractLiteral::MSet(members).transform(&move |x| match x {
                            AbstractLiteral::MSet(members) => {
                                let members = members
                                    .into_iter()
                                    .map(|x| match x {
                                        Literal::Bool(false) => Literal::Int(0),
                                        Literal::Bool(true) => Literal::Int(1),
                                        x => x,
                                    })
                                    .collect_vec();
                                AbstractLiteral::MSet(members)
                            }
                            x => x,
                        });
                        updates.push((k, Literal::AbstractLiteral(mset)));
                    }
                    Literal::AbstractLiteral(AbstractLiteral::Sequence(elems)) => {
                        let sequence =
                            AbstractLiteral::Sequence(elems).transform(&move |x| match x {
                                AbstractLiteral::Sequence(elems) => {
                                    let elems = elems
                                        .into_iter()
                                        .map(|x| match x {
                                            Literal::Bool(false) => Literal::Int(0),
                                            Literal::Bool(true) => Literal::Int(1),
                                            x => x,
                                        })
                                        .collect_vec();
                                    AbstractLiteral::Sequence(elems)
                                }
                                x => x,
                            });
                        updates.push((k, Literal::AbstractLiteral(sequence)));
                    }
                    Literal::AbstractLiteral(AbstractLiteral::Function(pairs)) => {
                        let function =
                            AbstractLiteral::Function(pairs).transform(&move |x| match x {
                                AbstractLiteral::Function(pairs) => {
                                    let pairs = pairs
                                        .into_iter()
                                        .map(|(key, value)| {
                                            let normalize = |x| match x {
                                                Literal::Bool(false) => Literal::Int(0),
                                                Literal::Bool(true) => Literal::Int(1),
                                                x => x,
                                            };
                                            (normalize(key), normalize(value))
                                        })
                                        .collect_vec();
                                    AbstractLiteral::Function(pairs)
                                }
                                x => x,
                            });
                        updates.push((k, Literal::AbstractLiteral(function)));
                    }
                    Literal::AbstractLiteral(AbstractLiteral::Relation(tuples)) => {
                        let relation =
                            AbstractLiteral::Relation(tuples).transform(&move |x| match x {
                                AbstractLiteral::Relation(tuples) => {
                                    let tuples = tuples
                                        .into_iter()
                                        .map(|fields| {
                                            fields
                                                .into_iter()
                                                .map(|x| match x {
                                                    Literal::Bool(false) => Literal::Int(0),
                                                    Literal::Bool(true) => Literal::Int(1),
                                                    x => x,
                                                })
                                                .collect_vec()
                                        })
                                        .collect_vec();
                                    AbstractLiteral::Relation(tuples)
                                }
                                x => x,
                            });
                        updates.push((k, Literal::AbstractLiteral(relation)));
                    }
                    Literal::AbstractLiteral(AbstractLiteral::Partition(parts)) => {
                        let partition =
                            AbstractLiteral::Partition(parts).transform(&move |x| match x {
                                AbstractLiteral::Partition(parts) => {
                                    let parts = parts
                                        .into_iter()
                                        .map(|part| {
                                            part.into_iter()
                                                .map(|x| match x {
                                                    Literal::Bool(false) => Literal::Int(0),
                                                    Literal::Bool(true) => Literal::Int(1),
                                                    x => x,
                                                })
                                                .collect_vec()
                                        })
                                        .collect_vec();
                                    AbstractLiteral::Partition(parts)
                                }
                                x => x,
                            });
                        updates.push((k, Literal::AbstractLiteral(partition)));
                    }
                    Literal::AbstractLiteral(AbstractLiteral::Permutation(cycles)) => {
                        let permutation =
                            AbstractLiteral::Permutation(cycles).transform(&move |x| match x {
                                AbstractLiteral::Permutation(cycles) => {
                                    let cycles = cycles
                                        .into_iter()
                                        .map(|cycle| {
                                            cycle
                                                .into_iter()
                                                .map(|x| match x {
                                                    Literal::Bool(false) => Literal::Int(0),
                                                    Literal::Bool(true) => Literal::Int(1),
                                                    x => x,
                                                })
                                                .collect_vec()
                                        })
                                        .collect_vec();
                                    AbstractLiteral::Permutation(cycles)
                                }
                                x => x,
                            });
                        updates.push((k, Literal::AbstractLiteral(permutation)));
                    }
                }
            }
        }

        for (k, v) in updates {
            let v = match v {
                Literal::AbstractLiteral(value) => {
                    Literal::AbstractLiteral(normalize_set_literal_order(value))
                }
                value => value,
            };
            solset.insert(k, v);
        }
    }

    // Remove duplicates and put solutions in a stable order for set-equality compares.
    normalized = normalized.into_iter().unique().collect();
    normalized.sort_by(solution_essence_cmp);
    normalized
}

fn solution_essence_cmp(
    lhs: &BTreeMap<Name, Literal>,
    rhs: &BTreeMap<Name, Literal>,
) -> std::cmp::Ordering {
    lhs.iter()
        .zip(rhs)
        .find_map(|((lhs_name, lhs_value), (rhs_name, rhs_value))| {
            let ordering = lhs_name.cmp(rhs_name);
            (ordering != std::cmp::Ordering::Equal)
                .then_some(ordering)
                .or_else(|| {
                    let ordering = lhs_value.essence_cmp(rhs_value);
                    (ordering != std::cmp::Ordering::Equal).then_some(ordering)
                })
        })
        .unwrap_or_else(|| lhs.len().cmp(&rhs.len()))
}

fn normalize_set_literal_order(value: AbstractLiteral<Literal>) -> AbstractLiteral<Literal> {
    value.transform(&|value| match value {
        AbstractLiteral::Set(mut members) => {
            members.sort_by(Literal::essence_cmp);
            AbstractLiteral::Set(members)
        }
        AbstractLiteral::MSet(mut members) => {
            members.sort_by(Literal::essence_cmp);
            AbstractLiteral::MSet(members)
        }
        AbstractLiteral::Function(mut pairs) => {
            pairs.sort_by(|(k1, _), (k2, _)| Literal::essence_cmp(k1, k2));
            AbstractLiteral::Function(pairs)
        }
        AbstractLiteral::Relation(mut tuples) => {
            tuples.sort_by(|a, b| {
                a.iter()
                    .zip(b.iter())
                    .map(|(x, y)| Literal::essence_cmp(x, y))
                    .find(|ord| *ord != std::cmp::Ordering::Equal)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            AbstractLiteral::Relation(tuples)
        }
        AbstractLiteral::Partition(mut parts) => {
            for part in parts.iter_mut() {
                part.sort_by(Literal::essence_cmp);
            }
            parts.sort_by(|a, b| {
                a.iter()
                    .zip(b.iter())
                    .map(|(x, y)| Literal::essence_cmp(x, y))
                    .find(|ord| *ord != std::cmp::Ordering::Equal)
                    .unwrap_or_else(|| a.len().cmp(&b.len()))
            });
            AbstractLiteral::Partition(parts)
        }
        value => value,
    })
}

fn maybe_truncate_serialised_json(serialised: String, test_stage: &str) -> String {
    if test_stage == "rewrite" {
        truncate_to_first_lines(&serialised, REWRITE_SERIALISED_JSON_MAX_LINES)
    } else {
        serialised
    }
}

fn truncate_to_first_lines(content: &str, max_lines: usize) -> String {
    content.lines().take(max_lines).join("\n")
}

pub fn truncate_to_first_chars(content: &str, max_chars: usize) -> String {
    match content.char_indices().nth(max_chars) {
        Some((idx, _)) => content[..idx].to_owned(),
        None => content.to_owned(),
    }
}

fn read_first_n_lines<P: AsRef<Path>>(filename: P, n: usize) -> io::Result<String> {
    let reader = BufReader::new(File::open(&filename)?);
    let lines = reader
        .lines()
        .chunks(n)
        .into_iter()
        .next()
        .unwrap()
        .collect::<Result<Vec<_>, _>>()?;
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(members: Vec<Literal>) -> Literal {
        Literal::AbstractLiteral(AbstractLiteral::Set(members))
    }

    #[test]
    fn solution_normalization_sorts_nested_set_members_by_essence_order() {
        let inner_one_two = set(vec![Literal::Int(2), Literal::Int(1)]);
        let inner_two = set(vec![Literal::Int(2)]);
        let mut oxide_solution = BTreeMap::new();
        oxide_solution.insert(
            Name::User("x".into()),
            set(vec![inner_two.clone(), inner_one_two.clone()]),
        );
        let mut conjure_solution = BTreeMap::new();
        conjure_solution.insert(
            Name::User("x".into()),
            set(vec![inner_one_two, inner_two.clone()]),
        );

        let normalized_oxide = normalize_solutions_for_comparison(&[oxide_solution]);
        let normalized_conjure = normalize_solutions_for_comparison(&[conjure_solution]);
        let expected = set(vec![inner_two, set(vec![Literal::Int(1), Literal::Int(2)])]);

        assert_eq!(normalized_oxide, normalized_conjure);
        assert_eq!(
            normalized_oxide[0].get(&Name::User("x".into())),
            Some(&expected)
        );
    }
}

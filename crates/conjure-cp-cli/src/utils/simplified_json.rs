//! Conjure's simplified JSON format for parameters and solutions.
//!
//! Matches Conjure `--output-format=json`: integers and booleans are JSON scalars, sets and
//! tuples are arrays, records are objects, and int-indexed matrices are objects whose keys are
//! the printed index values. Non-int-indexed matrices encode as arrays.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use anyhow::{anyhow, bail, Context as _};
use conjure_cp::ast::{
    AbstractLiteral, DeclarationPtr, DomainPtr, Field, GroundDomain, Literal, Moo, Name, Range,
};
use conjure_cp::Model;
use serde_json::{Map, Number, Value as JsonValue};

use crate::utils::json::sort_json_object;

/// Render solutions as a JSON array of assignment objects (Conjure `--solutions-in-one-file`).
pub fn solutions_to_simplified_json(
    solutions: &[BTreeMap<Name, Literal>],
) -> anyhow::Result<JsonValue> {
    let mut sorted = solutions.to_vec();
    sorted.sort_by(|lhs, rhs| solution_key_cmp(lhs, rhs));
    let mut items = Vec::with_capacity(sorted.len());
    for solution in &sorted {
        items.push(solution_to_simplified_json(solution)?);
    }
    Ok(JsonValue::Array(items))
}

/// Pretty-print solutions in Conjure's simplified JSON layout.
pub fn solutions_to_simplified_json_string(
    solutions: &[BTreeMap<Name, Literal>],
) -> anyhow::Result<String> {
    let value = solutions_to_simplified_json(solutions)?;
    Ok(format!("{}\n", serde_json::to_string_pretty(&value)?))
}

/// Parse a Conjure-style solutions JSON document (array, or a single object).
pub fn solutions_from_simplified_json(
    value: &JsonValue,
    domains: &BTreeMap<Name, DomainPtr>,
) -> anyhow::Result<Vec<BTreeMap<Name, Literal>>> {
    match value {
        JsonValue::Array(items) => items
            .iter()
            .map(|item| solution_from_simplified_json(item, domains))
            .collect(),
        JsonValue::Object(_) => Ok(vec![solution_from_simplified_json(value, domains)?]),
        _ => bail!("expected a JSON array or object of solutions"),
    }
}

/// Parse solutions JSON text.
pub fn solutions_from_simplified_json_str(
    text: &str,
    domains: &BTreeMap<Name, DomainPtr>,
) -> anyhow::Result<Vec<BTreeMap<Name, Literal>>> {
    let value: JsonValue = serde_json::from_str(text).context("parsing solutions JSON")?;
    solutions_from_simplified_json(&value, domains)
}

/// Render a single assignment as a simplified JSON object.
pub fn solution_to_simplified_json(
    solution: &BTreeMap<Name, Literal>,
) -> anyhow::Result<JsonValue> {
    let mut object = Map::new();
    for (name, value) in solution {
        object.insert(name.to_string(), literal_to_simplified_json(value)?);
    }
    Ok(sort_json_object(&JsonValue::Object(object), false))
}

/// Parse a single assignment object.
pub fn solution_from_simplified_json(
    value: &JsonValue,
    domains: &BTreeMap<Name, DomainPtr>,
) -> anyhow::Result<BTreeMap<Name, Literal>> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("each solution must be a JSON object"))?;
    let mut solution = BTreeMap::new();
    for (name, json_value) in object {
        let name = Name::user(name.as_str());
        let domain = domains.get(&name);
        let literal = literal_from_simplified_json(json_value, domain)
            .with_context(|| format!("parsing value for `{name}`"))?;
        solution.insert(name, literal);
    }
    Ok(solution)
}

/// Render a parameter file as a single simplified JSON object.
pub fn params_to_simplified_json(params: &BTreeMap<Name, Literal>) -> anyhow::Result<JsonValue> {
    solution_to_simplified_json(params)
}

/// Parse a parameter JSON object using given domains from the problem model.
pub fn params_from_simplified_json(
    value: &JsonValue,
    given_domains: &BTreeMap<Name, DomainPtr>,
) -> anyhow::Result<BTreeMap<Name, Literal>> {
    solution_from_simplified_json(value, given_domains)
}

/// Parse parameter JSON text.
pub fn params_from_simplified_json_str(
    text: &str,
    given_domains: &BTreeMap<Name, DomainPtr>,
) -> anyhow::Result<BTreeMap<Name, Literal>> {
    let value: JsonValue = serde_json::from_str(text).context("parsing parameter JSON")?;
    params_from_simplified_json(&value, given_domains)
}

/// Encode a literal in Conjure's simplified JSON format.
pub fn literal_to_simplified_json(literal: &Literal) -> anyhow::Result<JsonValue> {
    match literal {
        Literal::Bool(b) => Ok(JsonValue::Bool(*b)),
        Literal::Int(i) => Ok(JsonValue::Number(Number::from(*i))),
        Literal::AbstractLiteral(abs) => abstract_literal_to_simplified_json(abs),
    }
}

fn abstract_literal_to_simplified_json(
    abs: &AbstractLiteral<Literal>,
) -> anyhow::Result<JsonValue> {
    match abs {
        AbstractLiteral::Set(elems) | AbstractLiteral::MSet(elems) => elems_as_json_array(elems),
        AbstractLiteral::Tuple(elems) | AbstractLiteral::Sequence(elems) => {
            elems_as_json_array(elems)
        }
        AbstractLiteral::Record(fields) => {
            let mut object = Map::new();
            for field in fields {
                object.insert(
                    field.name.to_string(),
                    literal_to_simplified_json(&field.value)?,
                );
            }
            Ok(JsonValue::Object(object))
        }
        AbstractLiteral::Variant(field) => {
            let mut object = Map::new();
            object.insert(
                field.name.to_string(),
                literal_to_simplified_json(&field.value)?,
            );
            Ok(JsonValue::Object(object))
        }
        AbstractLiteral::Matrix(elems, index_domain) => {
            matrix_to_simplified_json(elems, index_domain.as_ref())
        }
        AbstractLiteral::Function(pairs) => function_to_simplified_json(pairs),
        AbstractLiteral::Relation(rows) => {
            let mut items = Vec::with_capacity(rows.len());
            for row in rows {
                items.push(elems_as_json_array(row)?);
            }
            Ok(JsonValue::Array(items))
        }
        AbstractLiteral::Partition(parts) => {
            let mut items = Vec::with_capacity(parts.len());
            for part in parts {
                items.push(elems_as_json_array(part)?);
            }
            Ok(JsonValue::Array(items))
        }
    }
}

fn matrix_to_simplified_json(
    elems: &[Literal],
    index_domain: &GroundDomain,
) -> anyhow::Result<JsonValue> {
    let GroundDomain::Int(_) = index_domain else {
        return elems_as_json_array(elems);
    };

    let indices: Vec<Literal> = match index_domain.values() {
        Ok(iter) => iter.collect(),
        Err(_) => return elems_as_json_array(elems),
    };

    if indices.len() != elems.len() {
        return elems_as_json_array(elems);
    }

    dictionary_from_pairs(indices.into_iter().zip(elems.iter().cloned()))
}

fn function_to_simplified_json(pairs: &[(Literal, Literal)]) -> anyhow::Result<JsonValue> {
    dictionary_from_pairs(pairs.iter().cloned())
}

fn dictionary_from_pairs(
    pairs: impl IntoIterator<Item = (Literal, Literal)>,
) -> anyhow::Result<JsonValue> {
    let mut object_entries = Vec::new();
    let mut array_entries = Vec::new();
    let mut all_keys_ok = true;

    for (key, value) in pairs {
        let key_json = literal_to_simplified_json(&key)?;
        let value_json = literal_to_simplified_json(&value)?;
        array_entries.push(JsonValue::Array(vec![key_json.clone(), value_json.clone()]));
        match &key_json {
            JsonValue::Bool(_) | JsonValue::Number(_) | JsonValue::String(_) => {
                object_entries.push((json_key_string(&key_json)?, value_json));
            }
            _ => {
                all_keys_ok = false;
            }
        }
    }

    if all_keys_ok {
        Ok(JsonValue::Object(object_entries.into_iter().collect()))
    } else {
        Ok(JsonValue::Array(array_entries))
    }
}

fn elems_as_json_array(elems: &[Literal]) -> anyhow::Result<JsonValue> {
    let mut items = Vec::with_capacity(elems.len());
    for elem in elems {
        items.push(literal_to_simplified_json(elem)?);
    }
    Ok(JsonValue::Array(items))
}

fn json_key_string(value: &JsonValue) -> anyhow::Result<String> {
    match value {
        JsonValue::Bool(b) => Ok(b.to_string()),
        JsonValue::Number(n) => Ok(n.to_string()),
        JsonValue::String(s) => Ok(s.clone()),
        _ => bail!("JSON object keys must be bool, number, or string"),
    }
}

/// Decode a simplified JSON value, using `domain` to disambiguate arrays and objects.
pub fn literal_from_simplified_json(
    value: &JsonValue,
    domain: Option<&DomainPtr>,
) -> anyhow::Result<Literal> {
    if let Some(domain) = domain {
        if let Some(ground) = domain.as_ground() {
            return literal_from_simplified_json_with_ground(value, ground);
        }
        // Domain lettings (e.g. `find x : R`) are unresolved until resolved through the symbol
        // table; resolve them so records/sets/etc. are not misread as matrices.
        if let Ok(ground) = domain.resolve() {
            return literal_from_simplified_json_with_ground(value, ground.as_ref());
        }
    }
    literal_from_simplified_json_unguided(value)
}

fn literal_from_simplified_json_with_ground(
    value: &JsonValue,
    domain: &GroundDomain,
) -> anyhow::Result<Literal> {
    match domain {
        GroundDomain::Bool => match value {
            JsonValue::Bool(b) => Ok(Literal::Bool(*b)),
            JsonValue::Number(n) => match n.as_i64() {
                Some(1) => Ok(Literal::Bool(true)),
                Some(0) => Ok(Literal::Bool(false)),
                _ => bail!("expected a boolean"),
            },
            _ => bail!("expected a boolean"),
        },
        GroundDomain::Int(_) => Ok(Literal::Int(json_to_i32(value)?)),
        GroundDomain::Set(_, inner) => {
            let JsonValue::Array(items) = value else {
                bail!("expected a JSON array for a set");
            };
            let mut elems = Vec::with_capacity(items.len());
            for item in items {
                elems.push(literal_from_simplified_json_with_ground(item, inner)?);
            }
            Ok(Literal::AbstractLiteral(AbstractLiteral::Set(elems)))
        }
        GroundDomain::MSet(_, inner) => {
            let JsonValue::Array(items) = value else {
                bail!("expected a JSON array for a multiset");
            };
            let mut elems = Vec::with_capacity(items.len());
            for item in items {
                elems.push(literal_from_simplified_json_with_ground(item, inner)?);
            }
            Ok(Literal::AbstractLiteral(AbstractLiteral::MSet(elems)))
        }
        GroundDomain::Tuple(inners) => {
            let JsonValue::Array(items) = value else {
                bail!("expected a JSON array for a tuple");
            };
            if items.len() != inners.len() {
                bail!(
                    "tuple arity mismatch: expected {}, got {}",
                    inners.len(),
                    items.len()
                );
            }
            let mut elems = Vec::with_capacity(items.len());
            for (item, inner) in items.iter().zip(inners) {
                elems.push(literal_from_simplified_json_with_ground(item, inner)?);
            }
            Ok(Literal::AbstractLiteral(AbstractLiteral::Tuple(elems)))
        }
        GroundDomain::Record(fields) => {
            let JsonValue::Object(object) = value else {
                bail!("expected a JSON object for a record");
            };
            let mut entries = Vec::with_capacity(fields.len());
            for field in fields {
                let key = field.name.to_string();
                let Some(item) = object.get(&key) else {
                    bail!("missing record field `{key}`");
                };
                entries.push(Field {
                    name: field.name.clone(),
                    value: literal_from_simplified_json_with_ground(item, field.value.as_ref())?,
                });
            }
            Ok(Literal::AbstractLiteral(AbstractLiteral::Record(entries)))
        }
        GroundDomain::Matrix(inner, index_domains) => {
            matrix_from_simplified_json(value, inner.as_ref(), index_domains)
        }
        GroundDomain::Sequence(_, inner) => match value {
            JsonValue::Array(items) => {
                let mut elems = Vec::with_capacity(items.len());
                for item in items {
                    elems.push(literal_from_simplified_json_with_ground(item, inner)?);
                }
                Ok(Literal::AbstractLiteral(AbstractLiteral::Sequence(elems)))
            }
            JsonValue::Object(object) => sequence_from_object(object, inner),
            _ => bail!("expected a JSON array for a sequence"),
        },
        GroundDomain::Function(_, from, to) => {
            function_from_simplified_json(value, from.as_ref(), to.as_ref())
        }
        GroundDomain::Empty(_) => bail!("cannot parse a value for an empty domain"),
        GroundDomain::Partition(_, _)
        | GroundDomain::Relation(_, _)
        | GroundDomain::Variant(_) => literal_from_simplified_json_unguided(value),
    }
}

fn matrix_from_simplified_json(
    value: &JsonValue,
    inner: &GroundDomain,
    index_domains: &[Moo<GroundDomain>],
) -> anyhow::Result<Literal> {
    let (first_index, rest) = index_domains
        .split_first()
        .ok_or_else(|| anyhow!("matrix domain has no index domains"))?;

    let elem_domain: GroundDomain = if rest.is_empty() {
        inner.clone()
    } else {
        GroundDomain::Matrix(Moo::new(inner.clone()), rest.to_vec())
    };

    match value {
        JsonValue::Object(object) => {
            let mut pairs = Vec::with_capacity(object.len());
            for (key, item) in object {
                let index = index_key_to_literal(key, first_index)?;
                let elem = literal_from_simplified_json_with_ground(item, &elem_domain)?;
                pairs.push((index, elem));
            }
            pairs.sort_by(|a, b| a.0.essence_cmp(&b.0));
            let keys: Vec<i32> = object
                .keys()
                .map(|k| k.parse::<i32>())
                .collect::<Result<Vec<_>, _>>()
                .unwrap_or_default();
            let elems: Vec<Literal> = pairs.into_iter().map(|(_, v)| v).collect();
            let index_domain = if let Ok(expected) = first_index.values() {
                let expected: Vec<_> = expected.collect();
                if expected.len() == elems.len() {
                    first_index.as_ref().clone()
                } else {
                    infer_int_index_domain(&keys)
                }
            } else {
                infer_int_index_domain(&keys)
            };
            Ok(Literal::AbstractLiteral(AbstractLiteral::Matrix(
                elems,
                index_domain.into(),
            )))
        }
        JsonValue::Array(items) => {
            let mut elems = Vec::with_capacity(items.len());
            for item in items {
                elems.push(literal_from_simplified_json_with_ground(item, &elem_domain)?);
            }
            let n = i32::try_from(elems.len()).context("matrix too large")?;
            let index_domain = GroundDomain::Int(vec![Range::Bounded(1, n)]);
            Ok(Literal::AbstractLiteral(AbstractLiteral::Matrix(
                elems,
                index_domain.into(),
            )))
        }
        _ => bail!("expected a JSON object or array for a matrix"),
    }
}

fn infer_int_index_domain(keys: &[i32]) -> GroundDomain {
    if keys.is_empty() {
        return GroundDomain::Int(vec![]);
    }
    let mut ints = keys.to_vec();
    ints.sort_unstable();
    let min = ints[0];
    let max = *ints.last().expect("non-empty");
    if max - min + 1 == ints.len() as i32 {
        GroundDomain::Int(vec![Range::Bounded(min, max)])
    } else {
        GroundDomain::Int(ints.into_iter().map(Range::Single).collect())
    }
}

fn index_key_to_literal(key: &str, index_domain: &GroundDomain) -> anyhow::Result<Literal> {
    match index_domain {
        GroundDomain::Bool => match key {
            "false" => Ok(Literal::Bool(false)),
            "true" => Ok(Literal::Bool(true)),
            _ => bail!("expected boolean matrix index key"),
        },
        _ => Ok(Literal::Int(key.parse::<i32>().with_context(|| {
            format!("expected integer matrix index key, got `{key}`")
        })?)),
    }
}

fn sequence_from_object(
    object: &Map<String, JsonValue>,
    inner: &GroundDomain,
) -> anyhow::Result<Literal> {
    let mut pairs = Vec::with_capacity(object.len());
    for (key, item) in object {
        let index: i32 = key
            .parse()
            .with_context(|| format!("sequence index `{key}` is not an integer"))?;
        pairs.push((
            index,
            literal_from_simplified_json_with_ground(item, inner)?,
        ));
    }
    pairs.sort_by_key(|(i, _)| *i);
    Ok(Literal::AbstractLiteral(AbstractLiteral::Sequence(
        pairs.into_iter().map(|(_, v)| v).collect(),
    )))
}

fn function_from_simplified_json(
    value: &JsonValue,
    from: &GroundDomain,
    to: &GroundDomain,
) -> anyhow::Result<Literal> {
    match value {
        JsonValue::Object(object) => {
            let mut pairs = Vec::with_capacity(object.len());
            for (key, item) in object {
                let domain_key = index_key_to_literal(key, from)?;
                let mapped = literal_from_simplified_json_with_ground(item, to)?;
                pairs.push((domain_key, mapped));
            }
            Ok(Literal::AbstractLiteral(AbstractLiteral::Function(pairs)))
        }
        JsonValue::Array(items) => {
            let mut pairs = Vec::with_capacity(items.len());
            for item in items {
                let JsonValue::Array(pair) = item else {
                    bail!("function array entries must be [from, to] pairs");
                };
                if pair.len() != 2 {
                    bail!("function array entries must be [from, to] pairs");
                }
                pairs.push((
                    literal_from_simplified_json_with_ground(&pair[0], from)?,
                    literal_from_simplified_json_with_ground(&pair[1], to)?,
                ));
            }
            Ok(Literal::AbstractLiteral(AbstractLiteral::Function(pairs)))
        }
        _ => bail!("expected a JSON object or array for a function"),
    }
}

fn literal_from_simplified_json_unguided(value: &JsonValue) -> anyhow::Result<Literal> {
    match value {
        JsonValue::Bool(b) => Ok(Literal::Bool(*b)),
        JsonValue::Number(_) | JsonValue::String(_) => Ok(Literal::Int(json_to_i32(value)?)),
        JsonValue::Array(items) => {
            let mut elems = Vec::with_capacity(items.len());
            for item in items {
                elems.push(literal_from_simplified_json_unguided(item)?);
            }
            Ok(Literal::AbstractLiteral(AbstractLiteral::Set(elems)))
        }
        JsonValue::Object(object) => {
            let all_int_keys = object.keys().all(|key| key.parse::<i32>().is_ok());
            if all_int_keys {
                let mut pairs = Vec::with_capacity(object.len());
                for (key, item) in object {
                    let index: i32 = key
                        .parse()
                        .with_context(|| format!("unguided object key `{key}` is not an integer"))?;
                    pairs.push((index, literal_from_simplified_json_unguided(item)?));
                }
                pairs.sort_by_key(|(i, _)| *i);
                let keys: Vec<i32> = pairs.iter().map(|(i, _)| *i).collect();
                let elems: Vec<_> = pairs.into_iter().map(|(_, v)| v).collect();
                return Ok(Literal::AbstractLiteral(AbstractLiteral::Matrix(
                    elems,
                    infer_int_index_domain(&keys).into(),
                )));
            }

            // Non-integer keys: treat as a record (Conjure simplified JSON).
            let mut entries = Vec::with_capacity(object.len());
            for (key, item) in object {
                entries.push(Field {
                    name: Name::user(key.as_str()),
                    value: literal_from_simplified_json_unguided(item)?,
                });
            }
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            Ok(Literal::AbstractLiteral(AbstractLiteral::Record(entries)))
        }
        JsonValue::Null => bail!("null is not a valid Essence literal"),
    }
}

fn json_to_i32(value: &JsonValue) -> anyhow::Result<i32> {
    match value {
        JsonValue::Number(n) => n
            .as_i64()
            .and_then(|i| i32::try_from(i).ok())
            .ok_or_else(|| anyhow!("expected a 32-bit integer")),
        JsonValue::String(s) => s
            .parse::<i32>()
            .with_context(|| format!("expected an integer string, got `{s}`")),
        _ => bail!("expected an integer"),
    }
}

fn solution_key_cmp(
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

/// Collect find/given domains from a model for JSON parsing guidance.
pub fn domains_from_model(model: &Model) -> BTreeMap<Name, DomainPtr> {
    let mut domains = BTreeMap::new();
    for (name, decl) in model.symbols().clone().into_iter() {
        if let Some(domain) = decl.domain() {
            domains.insert(name, domain);
        }
    }
    domains
}

/// Build a parameter model of value-lettings from simplified JSON assignments.
pub fn param_model_from_assignments(
    params: BTreeMap<Name, Literal>,
    given_domains: &BTreeMap<Name, DomainPtr>,
    context: std::sync::Arc<std::sync::RwLock<conjure_cp::context::Context<'static>>>,
) -> Model {
    let mut model = Model::new(context);
    for (name, literal) in params {
        let decl = match given_domains.get(&name) {
            Some(domain) => DeclarationPtr::new_value_letting_with_domain(
                name,
                literal.into(),
                domain.clone(),
            ),
            None => DeclarationPtr::new_value_letting(name, literal.into()),
        };
        model.symbols_mut().update_insert(decl);
    }
    model
}

/// Render pretty JSON with stable key order for golden comparisons.
pub fn canonical_simplified_json_string(value: &JsonValue) -> anyhow::Result<String> {
    let sorted = sort_json_object(value, true);
    Ok(format!("{}\n", serde_json::to_string_pretty(&sorted)?))
}

/// Append pretty JSON to a string buffer.
pub fn write_simplified_json(value: &JsonValue, out: &mut String) -> anyhow::Result<()> {
    write!(out, "{}", serde_json::to_string_pretty(value)?)?;
    out.push('\n');
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Domain, SetAttr};

    fn int_dom(lo: i32, hi: i32) -> DomainPtr {
        Domain::int(vec![Range::Bounded(lo, hi)])
    }

    #[test]
    fn round_trips_scalars_and_set() {
        let mut domains = BTreeMap::new();
        domains.insert(Name::user("x"), int_dom(1, 3));
        domains.insert(
            Name::user("s"),
            Domain::set(SetAttr::<i32>::default(), int_dom(1, 3)),
        );

        let json = serde_json::json!([{"s": [1, 3], "x": 2}]);
        let solutions = solutions_from_simplified_json(&json, &domains).unwrap();
        assert_eq!(solutions.len(), 1);
        assert_eq!(solutions[0].get(&Name::user("x")), Some(&Literal::Int(2)));
        let Literal::AbstractLiteral(AbstractLiteral::Set(elems)) =
            solutions[0].get(&Name::user("s")).unwrap()
        else {
            panic!("expected set");
        };
        assert_eq!(elems, &vec![Literal::Int(1), Literal::Int(3)]);

        let rendered = solutions_to_simplified_json(&solutions).unwrap();
        let again = solutions_from_simplified_json(&rendered, &domains).unwrap();
        assert_eq!(again, solutions);
    }

    #[test]
    fn round_trips_int_indexed_matrix() {
        let matrix_dom = Domain::matrix(int_dom(1, 3), vec![int_dom(1, 2)]);
        let mut domains = BTreeMap::new();
        domains.insert(Name::user("m"), matrix_dom);

        let json = serde_json::json!([{"m": {"1": 1, "2": 2}}]);
        let solutions = solutions_from_simplified_json(&json, &domains).unwrap();
        let rendered = solutions_to_simplified_json(&solutions).unwrap();
        let again = solutions_from_simplified_json(&rendered, &domains).unwrap();
        assert_eq!(again, solutions);
    }

    #[test]
    fn unguided_object_with_named_fields_parses_as_record() {
        let json = serde_json::json!({"a": true, "b": 3});
        let literal = literal_from_simplified_json(&json, None).unwrap();
        let Literal::AbstractLiteral(AbstractLiteral::Record(fields)) = literal else {
            panic!("expected record, got {literal:?}");
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, Name::user("a"));
        assert_eq!(fields[0].value, Literal::Bool(true));
        assert_eq!(fields[1].name, Name::user("b"));
        assert_eq!(fields[1].value, Literal::Int(3));
    }
}

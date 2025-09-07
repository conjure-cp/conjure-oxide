use serde_json::Value;

/// Compare two JSON values.
/// If the values are String, Number, or Bool, they are compared directly.
/// If the values are arrays, they are compared element-wise.
/// Otherwise, they are compared as strings.
fn json_value_cmp(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        (Value::String(a), Value::String(b)) => a.cmp(b),
        (Value::Number(a), Value::Number(b)) => {
            let af = a.as_f64().unwrap_or_default();
            let bf = b.as_f64().unwrap_or_default();
            af.total_cmp(&bf)
        }
        (Value::Array(a), Value::Array(b)) => {
            for (a, b) in a.iter().zip(b.iter()) {
                let cmp = json_value_cmp(a, b);
                if cmp != std::cmp::Ordering::Equal {
                    return cmp;
                }
            }
            std::cmp::Ordering::Equal
        }
        _ => a.to_string().cmp(&b.to_string()),
    }
}

/// Sort the "variables" field by name.
/// We have to do this separately because that field is not a JSON object, instead it's an array of tuples.
pub fn sort_json_variables(value: &Value) -> Value {
    match value {
        Value::Array(vars) => {
            let mut vars_sorted = vars.clone();
            vars_sorted.sort_by(json_value_cmp);
            Value::Array(vars_sorted)
        }
        _ => value.clone(),
    }
}

/// Recursively sorts the keys of all JSON objects within the provided JSON value.
///
/// serde_json will output JSON objects in an arbitrary key order.
/// this is normally fine, except in our use case we wouldn't want to update the expected output again and again.
/// so a consistent (sorted) ordering of the keys is desirable.
pub fn sort_json_object(value: &Value, sort_arrays: bool) -> Value {
    match value {
        Value::Object(obj) => {
            let mut ordered: Vec<(String, Value)> = obj
                .iter()
                .map(|(k, v)| {
                    if k == "variables" {
                        (k.clone(), sort_json_variables(v))
                    } else {
                        (k.clone(), sort_json_object(v, sort_arrays))
                    }
                })
                .collect();

            ordered.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(ordered.into_iter().collect())
        }
        Value::Array(arr) => {
            let mut arr: Vec<Value> = arr
                .iter()
                .map(|val| sort_json_object(val, sort_arrays))
                .collect();

            if sort_arrays {
                arr.sort_by(json_value_cmp);
            }

            Value::Array(arr)
        }
        _ => value.clone(),
    }
}

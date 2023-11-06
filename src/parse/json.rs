use crate::ast::Model;
use serde_json::Value;

pub fn parse_json(str: &String) -> Result<Model, String> {
    let v: Value = match serde_json::from_str(str) {
        Ok(v) => Ok(v),
        Err(e) => Err(e.to_string()),
    }?;
    let constraints: &Vec<Value> = match &v["mStatements"] {
        Value::Array(a) => Ok(a),
        _ => Err("Invalid JSON"),
    }?;

    let mut m = Model::new();
    for c in constraints {
        let obj = match c {
            Value::Object(obj) => Ok(obj),
            _ => Err("Invalid JSON"),
        }?;
        println!("{:?}", c);
    }

    Ok(m)
}

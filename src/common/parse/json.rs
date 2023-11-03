use crate::common::ast::Model;
use json::JsonValue;

pub fn parse_json(str: &String) -> Result<Model, String> {
    let v = match json::parse(str) {
        Ok(v) => Ok(v),
        Err(err) => Err(format!("{:?}", err)),
    }?;
    let constraints: &Vec<JsonValue> = match &v["mStatements"] {
        JsonValue::Array(a) => Ok(a),
        _ => Err("Invalid JSON"),
    }?;

    let mut m = Model::new();
    for c in constraints {
        let obj = match c {
            JsonValue::Object(obj) => Ok(obj),
            _ => Err("Invalid JSON"),
        }?;
        println!("{:?}", c);
    }

    Ok(m)
}

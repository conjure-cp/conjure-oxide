use crate::common::ast::Model;
use json::JsonValue;

pub fn parse_json(v: &JsonValue) -> Result<Model, String> {
    let mut m = Model::new();
    if let JsonValue::Array(constraints) = &v["mStatements"] {
        for constraint in constraints {
            println!("{}", constraint);
        }
    } else {
        return Err(String::from("JSON is invalid"))
    }

    Ok(m)
}

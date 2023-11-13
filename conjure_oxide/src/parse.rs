use crate::ast::Model;
use crate::error::Error;
use serde_json::Value as JsonValue;

pub fn parse_json(str: &String) -> Result<Model, Error> {
    let v: JsonValue = serde_json::from_str(str)?;
    let constraints: &Vec<JsonValue> = match &v["mStatements"] {
        JsonValue::Array(a) => Ok(a),
        _ => Err(Error::ParseError("mStatements is not an array".to_owned())),
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

impl Model {
    pub fn from_json(str: &String) -> Result<Model, Error> {
        parse_json(str)
    }
}

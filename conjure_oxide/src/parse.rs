use crate::ast::Model;
use crate::error::Error;
use serde_json::Value as JsonValue;

use Error::ModelConstructError as CError;

pub fn parse_json(str: &String) -> Result<Model, Error> {
    let mut m = Model::new();
    let v: JsonValue = serde_json::from_str(str)?;
    let constraints = v["mStatements"]
        .as_array()
        .ok_or(CError("mStatements is not an array".to_owned()))?;

    for con in constraints {
        let obj = con
            .as_object()
            .ok_or(CError("mStatements contains a non-object".to_owned()))?;
        let entry = obj
            .iter()
            .next()
            .ok_or(CError("mStatements contains an empty object".to_owned()))?;
        println!("{:?}", entry);
    }

    Ok(m)
}

impl Model {
    pub fn from_json(str: &String) -> Result<Model, Error> {
        parse_json(str)
    }
}

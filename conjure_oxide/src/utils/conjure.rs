use crate::parse::model_from_json;
use crate::solvers::minion::MinionModel;
use crate::solvers::FromConjureModel;
use crate::utils::json::sort_json_object;
use crate::{Error as ParseErr, Error};
use conjure_core::ast::Model;
use minion_rs::ast::{Constant, VarName};
use minion_rs::run_minion;
use serde_json::{Map, Value as JsonValue};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Mutex;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum EssenceParseError {
    #[error("Error running conjure pretty: {0}")]
    ConjurePrettyError(String),
    #[error("Error parsing essence file: {0}")]
    ParseError(ParseErr),
}

impl From<ParseErr> for EssenceParseError {
    fn from(e: ParseErr) -> Self {
        EssenceParseError::ParseError(e)
    }
}

pub fn parse_essence_file(path: &str, filename: &str) -> Result<Model, EssenceParseError> {
    let mut cmd = std::process::Command::new("conjure");
    let output = match cmd
        .arg("pretty")
        .arg("--output-format=astjson")
        .arg(format!("{path}/{filename}.essence"))
        .output()
    {
        Ok(output) => output,
        Err(e) => return Err(EssenceParseError::ConjurePrettyError(e.to_string())),
    };

    if !output.status.success() {
        let stderr_string = String::from_utf8(output.stderr)
            .unwrap_or("stderr is not a valid UTF-8 string".to_string());
        return Err(EssenceParseError::ConjurePrettyError(stderr_string));
    }

    let astjson = match String::from_utf8(output.stdout) {
        Ok(astjson) => astjson,
        Err(e) => {
            return Err(EssenceParseError::ConjurePrettyError(format!(
                "Error parsing output from conjure: {:#?}",
                e
            )))
        }
    };

    let parsed_model = model_from_json(&astjson)?;
    Ok(parsed_model)
}

pub fn get_minion_solutions(
    model: Model,
) -> Result<Vec<HashMap<VarName, Constant>>, anyhow::Error> {
    static ALL_SOLUTIONS: Mutex<Vec<HashMap<VarName, Constant>>> = Mutex::new(vec![]);

    fn callback(solutions: HashMap<VarName, Constant>) -> bool {
        let mut guard = match ALL_SOLUTIONS.lock() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Error getting lock on ALL_SOLUTIONS: {}", e);
                return false;
            }
        };

        guard.push(solutions);
        true
    }

    println!("Building Minion model...");
    let minion_model = MinionModel::from_conjure(model)?;

    println!("Running Minion...");
    match run_minion(minion_model, callback) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Error running Minion: {}", e);
            return Err(anyhow::anyhow!("Error running Minion: {}", e));
        }
    };

    let guard = match ALL_SOLUTIONS.lock() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Error getting lock on ALL_SOLUTIONS: {}", e);
            return Err(anyhow::anyhow!(
                "Error getting lock on ALL_SOLUTIONS: {}",
                e
            ));
        }
    };
    Ok(guard.deref().clone())
}

pub fn minion_solutions_to_json(solutions: Vec<HashMap<VarName, Constant>>) -> JsonValue {
    let mut json_solutions = Vec::new();
    for solution in solutions {
        let mut json_solution = Map::new();
        for (var_name, constant) in solution {
            let serialized_constant = match constant {
                Constant::Integer(i) => JsonValue::Number(i.into()),
                Constant::Bool(b) => JsonValue::Bool(b),
            };
            json_solution.insert(var_name.to_string(), serialized_constant);
        }
        json_solutions.push(JsonValue::Object(json_solution));
    }
    let ans = JsonValue::Array(json_solutions);
    sort_json_object(&ans)
}

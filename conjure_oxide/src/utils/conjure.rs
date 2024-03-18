use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value as JsonValue};
use thiserror::Error as ThisError;

use crate::ast::{Constant, Name};
use crate::Error as ParseErr;
use crate::Model;
use crate::parse::model_from_json;
use crate::solver::{Solver, SolverAdaptor};
use crate::solver::adaptors::Minion;
use crate::utils::json::sort_json_object;

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

pub fn get_minion_solutions(model: Model) -> Result<Vec<HashMap<Name, Constant>>, anyhow::Error> {
    let solver = Solver::new(Minion::new());

    println!("Building Minion model...");
    let solver = solver.load_model(model)?;

    println!("Running Minion...");

    let all_solutions_ref = Arc::new(Mutex::<Vec<HashMap<Name, Constant>>>::new(vec![]));
    let all_solutions_ref_2 = all_solutions_ref.clone();
    #[allow(clippy::unwrap_used)]
    solver.solve(Box::new(move |sols| {
        let mut all_solutions = (*all_solutions_ref_2).lock().unwrap();
        (*all_solutions).push(sols);
        true
    }))?;

    #[allow(clippy::unwrap_used)]
    let sols = (*all_solutions_ref).lock().unwrap();
    Ok((*sols).clone())
}

pub fn minion_solutions_to_json(solutions: &Vec<HashMap<Name, Constant>>) -> JsonValue {
    let mut json_solutions = Vec::new();
    for solution in solutions {
        let mut json_solution = Map::new();
        for (var_name, constant) in solution {
            let serialized_constant = match constant {
                Constant::Int(i) => JsonValue::Number((*i).into()),
                Constant::Bool(b) => JsonValue::Bool(*b),
            };
            json_solution.insert(var_name.to_string(), serialized_constant);
        }
        json_solutions.push(JsonValue::Object(json_solution));
    }
    let ans = JsonValue::Array(json_solutions);
    sort_json_object(&ans, true)
}

use crate::parse::model_from_json;
use crate::solvers::minion::MinionModel;
use crate::solvers::FromConjureModel;
use crate::utils::json::sort_json_object;
use crate::Error as ParseErr;
use conjure_core::ast::Model;
use minion_rs::ast::{Constant, VarName};
use minion_rs::run_minion;
use serde_json::{Map, Value as JsonValue};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Condvar, Mutex};
use thiserror::Error as ThisError;

static ALL_SOLUTIONS: Mutex<Vec<HashMap<VarName, Constant>>> = Mutex::new(vec![]);
static LOCK: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

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
    fn callback(solutions: HashMap<VarName, Constant>) -> bool {
        match ALL_SOLUTIONS.lock() {
            Ok(mut guard) => {
                guard.push(solutions);
                true
            }
            Err(e) => {
                eprintln!("Error getting lock on ALL_SOLUTIONS: {}", e);
                false
            }
        }
    }

    println!("Building Minion model...");
    let minion_model = MinionModel::from_conjure(model)?;

    // @niklasdewally would be able to explain this better
    // We use a condvar to keep a lock on the ALL_SOLUTIONS mutex until it goes out of scope
    // So, no other threads can mutate ALL_SOLUTIONS while we're running Minion, only our callback can
    let (lock, condvar) = &LOCK;
    #[allow(clippy::unwrap_used)] // If the mutex is poisoned, we want to panic anyway
    let mut _lock_guard = condvar
        .wait_while(lock.lock().unwrap(), |locked| *locked)
        .unwrap();

    *_lock_guard = true;

    println!("Running Minion...");
    match run_minion(minion_model, callback) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Error running Minion: {}", e);
            return Err(anyhow::anyhow!("Error running Minion: {}", e));
        }
    };

    let ans = match ALL_SOLUTIONS.lock() {
        Ok(mut guard) => {
            let ans = guard.deref().clone();
            guard.clear(); // Clear the solutions for the next test
            ans
        }
        Err(e) => {
            eprintln!("Error getting lock on ALL_SOLUTIONS: {}", e);
            return Err(anyhow::anyhow!(
                "Error getting lock on ALL_SOLUTIONS: {}",
                e
            ));
        }
    };

    // Release the lock and wake the next waiting thread
    *_lock_guard = false;
    std::mem::drop(_lock_guard);
    condvar.notify_one();

    Ok(ans)
}

pub fn minion_solutions_to_json(solutions: &Vec<HashMap<VarName, Constant>>) -> JsonValue {
    let mut json_solutions = Vec::new();
    for solution in solutions {
        let mut json_solution = Map::new();
        for (var_name, constant) in solution {
            let serialized_constant = match constant {
                Constant::Integer(i) => JsonValue::Number((*i).into()),
                Constant::Bool(b) => JsonValue::Bool(*b),
            };
            json_solution.insert(var_name.to_string(), serialized_constant);
        }
        json_solutions.push(JsonValue::Object(json_solution));
    }
    let ans = JsonValue::Array(json_solutions);
    sort_json_object(&ans, true)
}

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::string::ToString;
use std::sync::{Arc, Mutex, RwLock};

use conjure_core::ast::{Literal, Name};
use conjure_core::context::Context;
use conjure_core::solver;
use conjure_core::solver::adaptors::SAT;
use rand::Rng as _;
use serde_json::{from_str, Map, Value as JsonValue};
use thiserror::Error as ThisError;

use std::fs::File;

use crate::model_from_json;
use crate::solver::adaptors::Minion;
use crate::solver::Solver;
use crate::utils::json::sort_json_object;
use crate::Error as ParseErr;
use crate::Model;

use glob::glob;

#[derive(Debug, ThisError)]
pub enum EssenceParseError {
    #[error("Error running conjure pretty: {0}")]
    ConjurePrettyError(String),
    #[error("Error running conjure solve: {0}")]
    ConjureSolveError(String),
    #[error("Error parsing essence file: {0}")]
    ParseError(ParseErr),
    #[error("Error parsing Conjure solutions file: {0}")]
    ConjureSolutionsError(String),
    #[error("No solutions file for {0}")]
    ConjureNoSolutionsFile(String),
}

impl From<ParseErr> for EssenceParseError {
    fn from(e: ParseErr) -> Self {
        EssenceParseError::ParseError(e)
    }
}

pub fn parse_essence_file(
    path: &str,
    filename: &str,
    extension: &str,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Model, EssenceParseError> {
    let mut cmd = std::process::Command::new("conjure");
    let output = match cmd
        .arg("pretty")
        .arg("--output-format=astjson")
        .arg(format!("{path}/{filename}.{extension}"))
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

    let parsed_model = model_from_json(&astjson, context)?;
    Ok(parsed_model)
}

pub fn get_minion_solutions(
    model: Model,
    num_sols: i32,
) -> Result<Vec<BTreeMap<Name, Literal>>, anyhow::Error> {
    let solver = Solver::new(Minion::new());
    println!("Building Minion model...");
    let solver = solver.load_model(model)?;

    println!("Running Minion...");

    let all_solutions_ref = Arc::new(Mutex::<Vec<BTreeMap<Name, Literal>>>::new(vec![]));
    let all_solutions_ref_2 = all_solutions_ref.clone();
    let solver = if num_sols > 0 {
        let sols_left = Mutex::new(num_sols);

        #[allow(clippy::unwrap_used)]
        solver
            .solve(Box::new(move |sols| {
                let mut all_solutions = (*all_solutions_ref_2).lock().unwrap();
                (*all_solutions).push(sols.into_iter().collect());
                let mut sols_left = sols_left.lock().unwrap();
                *sols_left -= 1;

                *sols_left != 0
            }))
            .unwrap()
    } else {
        #[allow(clippy::unwrap_used)]
        solver
            .solve(Box::new(move |sols| {
                let mut all_solutions = (*all_solutions_ref_2).lock().unwrap();
                (*all_solutions).push(sols.into_iter().collect());
                true
            }))
            .unwrap()
    };

    solver.save_stats_to_context();

    #[allow(clippy::unwrap_used)]
    let sols = (*all_solutions_ref).lock().unwrap();

    Ok((*sols).clone())
}

pub fn get_sat_solutions(
    model: Model,
    num_sols: i32,
) -> Result<Vec<BTreeMap<Name, Literal>>, anyhow::Error> {
    let mut sols: Vec<BTreeMap<Name, Literal>> = Vec::new();

    // let solver = Solver::new(SAT::default());
    // println!("Building SAT model...");
    // let solver = solver.load_model(model);
    // println!("Running Minion...");

    let mut solver: SAT = SAT::default();
    // solver.get_sat_solution(model.clone());
    for _i in 0..num_sols + 1 {
        // should always be run with num_sols = 1
        let solution = solver.get_sat_solution(model.clone());
        // println!(
        //     "\n------------------------solution #{} done------------------------\n",
        //     i + 1
        // );
        sols.push(solution);
    }
    Ok(sols)
}

#[allow(clippy::unwrap_used)]
pub fn get_solutions_from_conjure(
    essence_file: &str,
) -> Result<Vec<BTreeMap<Name, Literal>>, EssenceParseError> {
    // this is ran in parallel, and we have no guarantee by rust that invocations to this function
    // don't share the same tmp dir.
    let mut rng = rand::thread_rng();
    let rand: i8 = rng.gen();

    let mut tmp_dir = std::env::temp_dir();
    tmp_dir.push(Path::new(&rand.to_string()));

    let mut cmd = std::process::Command::new("conjure");
    let output = cmd
        .arg("solve")
        .arg("--output-format=json")
        .arg("--solutions-in-one-file")
        .arg("--number-of-solutions=all")
        .arg("--copy-solutions=no")
        .arg("-o")
        .arg(&tmp_dir)
        .arg(essence_file)
        .output()
        .map_err(|e| EssenceParseError::ConjureSolveError(e.to_string()))?;

    if !output.status.success() {
        return Err(EssenceParseError::ConjureSolveError(format!(
            "conjure solve exited with failure: {}",
            String::from_utf8(output.stderr).unwrap()
        )));
    }

    let solutions_files: Vec<_> = glob(&format!("{}/*.solutions.json", tmp_dir.display()))
        .unwrap()
        .collect();

    if solutions_files.is_empty() {
        return Err(EssenceParseError::ConjureNoSolutionsFile(
            tmp_dir.display().to_string(),
        ));
    }

    let solutions_file = solutions_files[0].as_ref().unwrap();
    let mut file = File::open(solutions_file).unwrap();

    let mut json_str = String::new();
    file.read_to_string(&mut json_str).unwrap();
    let mut json: JsonValue =
        from_str(&json_str).map_err(|e| EssenceParseError::ConjureSolutionsError(e.to_string()))?;
    json.sort_all_objects();

    let solutions = json
        .as_array()
        .ok_or(EssenceParseError::ConjureSolutionsError(
            "expected solutions to be an array".to_owned(),
        ))?;

    let mut solutions_set: Vec<BTreeMap<Name, Literal>> = Vec::new();

    for solution in solutions {
        let mut solution_map = BTreeMap::new();
        let solution = solution
            .as_object()
            .ok_or(EssenceParseError::ConjureSolutionsError(
                "invalid json".to_owned(),
            ))?;
        for (name, value) in solution {
            let name = Name::UserName(name.to_owned());
            let value = match value {
                JsonValue::Bool(b) => Ok(Literal::Bool(*b)),
                JsonValue::Number(n) => Ok(Literal::Int(n.as_i64().unwrap().try_into().unwrap())),
                a => Err(EssenceParseError::ConjureSolutionsError(
                    format!("expected constant, got {}", a).to_owned(),
                )),
            }?;
            solution_map.insert(name, value);
        }

        if !solution.is_empty() {
            solutions_set.push(solution_map);
        }
    }

    Ok(solutions_set)
}

pub fn minion_solutions_to_json(solutions: &Vec<BTreeMap<Name, Literal>>) -> JsonValue {
    let mut json_solutions = Vec::new();
    for solution in solutions {
        let mut json_solution = Map::new();
        for (var_name, constant) in solution {
            let serialized_constant = match constant {
                Literal::Int(i) => JsonValue::Number((*i).into()),
                Literal::Bool(b) => JsonValue::Bool(*b),
            };
            json_solution.insert(var_name.to_string(), serialized_constant);
        }
        json_solutions.push(JsonValue::Object(json_solution));
    }
    let ans = JsonValue::Array(json_solutions);
    sort_json_object(&ans, true)
}

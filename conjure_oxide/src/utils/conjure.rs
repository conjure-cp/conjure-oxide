use std::collections::BTreeMap;
use std::rc::Rc;
use std::string::ToString;
use std::sync::{Arc, Mutex, RwLock};

use conjure_core::ast::{Literal, Name};
use conjure_core::bug;
use conjure_core::context::Context;

use conjure_core::solver::adaptors::SAT;
use serde_json::{Map, Value as JsonValue};

use itertools::Itertools as _;
use tempfile::tempdir;

use crate::parse_essence_file;
use crate::solver::adaptors::Minion;
use crate::solver::Solver;
use crate::utils::json::sort_json_object;
use crate::EssenceParseError;
use crate::Model;

use glob::glob;

pub fn get_minion_solutions(
    model: Model,
    num_sols: i32,
) -> Result<Vec<BTreeMap<Name, Literal>>, anyhow::Error> {
    let solver = Solver::new(Minion::new());
    println!("Building Minion model...");

    // for later...
    let symbols_rc = Rc::clone(model.as_submodel().symbols_ptr_unchecked());

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
    let mut sols_guard = (*all_solutions_ref).lock().unwrap();
    let sols = &mut *sols_guard;
    let symbols = symbols_rc.borrow();

    let names = symbols.clone().into_iter().map(|x| x.0).collect_vec();
    let representations = names
        .into_iter()
        .filter_map(|x| symbols.representations_for(&x).map(|repr| (x, repr)))
        .filter_map(|(name, reprs)| {
            if reprs.is_empty() {
                return None;
            }
            assert!(
                reprs.len() <= 1,
                "multiple representations for a variable is not yet implemented"
            );

            assert_eq!(
                reprs[0].len(),
                1,
                "nested representations are not yet implemented"
            );
            Some((name, reprs[0][0].clone()))
        })
        .collect_vec();

    for sol in sols.iter_mut() {
        for (name, representation) in representations.iter() {
            let value = representation.value_up(sol).unwrap();
            sol.insert(name.clone(), value);
        }

        // remove represented variables
        *sol = sol
            .clone()
            .into_iter()
            .filter(|(name, _)| !matches!(name, Name::RepresentedName(_, _, _)))
            .collect();
    }

    Ok(sols.clone().into_iter().filter(|x| !x.is_empty()).collect())
}

pub fn get_sat_solutions(
    model: Model,
    num_sols: i32,
) -> Result<Vec<BTreeMap<Name, Literal>>, anyhow::Error> {
    let solver = Solver::new(SAT::default());
    println!("Building SAT model...");
    let solver = solver.load_model(model)?;

    println!("Running SAT...");

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

#[allow(clippy::unwrap_used)]
pub fn get_solutions_from_conjure(
    essence_file: &str,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Vec<BTreeMap<Name, Literal>>, EssenceParseError> {
    let tmp_dir = tempdir().unwrap();

    let mut cmd = std::process::Command::new("conjure");
    let output = cmd
        .arg("solve")
        .arg("--number-of-solutions=all")
        .arg("--copy-solutions=no")
        .arg("-o")
        .arg(tmp_dir.path())
        .arg(essence_file)
        .output()
        .map_err(|e| EssenceParseError::ConjureSolveError(e.to_string()))?;

    if !output.status.success() {
        return Err(EssenceParseError::ConjureSolveError(format!(
            "conjure solve exited with failure: {}",
            String::from_utf8(output.stderr).unwrap()
        )));
    }

    let solutions_files: Vec<_> = glob(&format!("{}/*.solution", tmp_dir.path().display()))
        .unwrap()
        .collect();

    let mut solutions_set = vec![];
    for solutions_file in solutions_files {
        let solutions_file = solutions_file.unwrap();
        let model = parse_essence_file(solutions_file.to_str().unwrap(), Arc::clone(&context))
            .expect("conjure solutions files to be parsable");

        let mut solutions = BTreeMap::new();
        for (name, decl) in model.as_submodel().symbols().clone().into_iter() {
            match decl.kind() {
                conjure_core::ast::DeclarationKind::ValueLetting(expression) => {
                    let literal = expression
                        .clone()
                        .to_literal()
                        .expect("lettings in a solution should only contain literals");
                    solutions.insert(name, literal);
                }
                _ => {
                    bug!("only expect value letting declarations in solutions")
                }
            }
        }
        solutions_set.push(solutions);
    }

    Ok(solutions_set
        .into_iter()
        .filter(|x| !x.is_empty())
        .collect())
}

pub fn solutions_to_json(solutions: &Vec<BTreeMap<Name, Literal>>) -> JsonValue {
    let mut json_solutions = Vec::new();
    for solution in solutions {
        let mut json_solution = Map::new();
        for (var_name, constant) in solution {
            let serialized_constant = serde_json::to_value(constant).unwrap();
            json_solution.insert(var_name.to_string(), serialized_constant);
        }
        json_solutions.push(JsonValue::Object(json_solution));
    }
    let ans = JsonValue::Array(json_solutions);
    sort_json_object(&ans, true)
}

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::string::ToString;
use std::sync::{Arc, Mutex, RwLock};

use conjure_cp::ast::{DeclarationKind, DeclarationPtr, Literal, Name};
use conjure_cp::bug;
use conjure_cp::context::Context;

use serde_json::{Map, Value as JsonValue};

use itertools::Itertools as _;
use tempfile::tempdir;

use crate::utils::json::sort_json_object;
use conjure_cp::Model;
use conjure_cp::parse::tree_sitter::parse_essence_file;
use conjure_cp::solver::Solver;

use glob::glob;

use conjure_cp::ast::categories::{Category, CategoryOf};
use conjure_cp::representation::util::try_up;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

pub fn get_solutions(
    solver: Solver,
    model: Model,
    num_sols: i32,
    solver_input_file: &Option<PathBuf>,
) -> Result<Vec<BTreeMap<Name, Literal>>, anyhow::Error> {
    let adaptor_name = solver.get_name();

    eprintln!("Building {adaptor_name} model...");

    // Create for later since we consume the model when loading it
    let symbols_ptr = model.symbols_ptr_unchecked().clone();

    let solver = solver.load_model(model)?;

    if let Some(solver_input_file) = solver_input_file {
        eprintln!(
            "Writing solver input file to {}",
            solver_input_file.display()
        );
        let file = Box::new(std::fs::File::create(solver_input_file)?);
        solver.write_solver_input_file(&mut (file as Box<dyn std::io::Write>))?;
    }

    eprintln!("Running {adaptor_name}...");

    // Create two arcs, one to pass into the solver callback, one to get solutions out later
    let all_solutions_ref = Arc::new(Mutex::<Vec<HashMap<Name, Literal>>>::new(vec![]));
    let all_solutions_ref_2 = all_solutions_ref.clone();

    let solver = if num_sols > 0 {
        // Get num_sols solutions
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
        // Get all solutions
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

    // Get the collections of solutions and model symbols
    #[allow(clippy::unwrap_used)]
    let sols_guard = (*all_solutions_ref).lock().unwrap();
    let symbols = symbols_ptr.read();

    // for (_, sym) in symbols.iter_local() {
    //     println!("{sym:#?}");
    //     for (repr_name, repr_state) in sym.reprs().iter() {
    //         println!("Representation '{repr_name}':");
    //         println!("{repr_state:#?}");
    //         println!();
    //     }
    //     println!();
    //     println!();
    // }

    // TODO: do we need to collect quantified vars too?
    let decision_vars: Vec<DeclarationPtr> = symbols
        .iter_local()
        .filter(|(n, d)| matches!(n, Name::User(..)) && d.category_of() >= Category::Decision)
        .map(|(_, d)| d)
        .cloned()
        .collect();

    let ans = sols_guard
        .iter()
        .filter_map(|sol| {
            // println!("Solution:");
            // println!("{:#?}", sol);
            // println!();
            let mut ans = BTreeMap::<Name, Literal>::new();

            for decl in decision_vars.iter() {
                // Look up the variable or go up via its representation
                // TODO: we should check that all reprs give the same value...
                let res = try_up(decl.clone(), sol).unwrap();
                ans.insert(decl.name().clone(), res);
            }

            if ans.is_empty() { None } else { Some(ans) }
        })
        .collect();
    Ok(ans)
}

#[allow(clippy::unwrap_used)]
pub fn get_solutions_from_conjure(
    essence_file: &str,
    param_file: Option<&str>,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Vec<BTreeMap<Name, Literal>>, anyhow::Error> {
    let tmp_dir = tempdir()?;

    let mut cmd = std::process::Command::new("conjure");

    cmd.arg("solve")
        .arg("--number-of-solutions=all")
        .arg("--copy-solutions=no")
        .arg("-o")
        .arg(tmp_dir.path())
        .arg(essence_file);

    if let Some(file) = param_file {
        cmd.arg(file);
    }

    let output = cmd.output()?;

    if !output.status.success() {
        let stderr =
            String::from_utf8(output.stderr).unwrap_or_else(|e| e.utf8_error().to_string());
        return Err(anyhow::Error::msg(format!(
            "Error: `conjure solve` exited with code {}; stderr: {}",
            output.status, stderr
        )));
    }

    let solutions_files: Vec<_> =
        glob(&format!("{}/*.solution", tmp_dir.path().display()))?.collect();

    let solutions_set: Vec<_> = solutions_files
        .par_iter()
        .map(|solutions_file| {
            let solutions_file = solutions_file.as_ref().unwrap();
            let model = parse_essence_file(solutions_file.to_str().unwrap(), Arc::clone(&context))
                .expect("conjure solutions files to be parsable");

            let mut solutions = BTreeMap::new();
            for (name, decl) in model.symbols().clone().into_iter() {
                match &decl.kind() as &DeclarationKind {
                    conjure_cp::ast::DeclarationKind::ValueLetting(expression, _) => {
                        let literal = expression
                            .clone()
                            .into_literal()
                            .expect("lettings in a solution should only contain literals");
                        solutions.insert(name, literal);
                    }
                    _ => {
                        bug!("only expect value letting declarations in solutions")
                    }
                }
            }
            solutions
        })
        .collect();

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

pub fn solutions_to_essence(solutions: &Vec<BTreeMap<Name, Literal>>) -> Vec<String> {
    let mut ans = Vec::new();
    for solution in solutions {
        let mut sol = Vec::new();
        for (name, value) in solution {
            sol.push(format!("letting {name} be {value}"));
        }
        ans.push(sol.join("\n"));
    }
    ans
}

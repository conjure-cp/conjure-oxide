use conjure_cp::ast::{DeclarationKind, DeclarationPtr, Literal, Name};
use conjure_cp::bug;
use conjure_cp::context::Context;
use conjure_cp::solver::adaptors::Minion;
use conjure_cp::solver::adaptors::Sat;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::string::ToString;
use std::sync::{Arc, Mutex, RwLock};

use conjure_cp::solver::SolverFamily;
use itertools::Itertools as _;
use serde_json::{Map, Value as JsonValue};
use tempfile::tempdir;

use crate::utils::json::sort_json_object;
use conjure_cp::Model;
use conjure_cp::ast::{Atom, Expression, Metadata, Moo};
use conjure_cp::parse::tree_sitter::parse_essence_file;
use conjure_cp::rule_engine::rewrite_naive;
use conjure_cp::solver::{Solver, SolverAdaptor};

use glob::glob;

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

pub fn get_solutions(
    solver: SolverFamily,
    model: Model,
    num_sols: i32,
    solver_input_file: &Option<PathBuf>,
) -> Result<Vec<BTreeMap<Name, Literal>>, anyhow::Error> {
    if let Some(Expression::DominanceRelation(_, dom_rel)) = model.dominance.clone() {
        get_solutions_with_dominance(solver, model, num_sols, solver_input_file, &dom_rel)
    } else {
        match solver {
            SolverFamily::Sat => get_solutions_no_dominance(Sat::default(), model, num_sols, solver_input_file),
            SolverFamily::Minion => get_solutions_no_dominance(Minion::default(), model, num_sols, solver_input_file),
        }
    }
}

pub fn get_solutions_with_dominance(
    solver: SolverFamily,
    mut model: Model,
    _num_sols: i32,
    solver_input_file: &Option<PathBuf>,
    dom_rel: &Expression,
) -> Result<Vec<BTreeMap<Name, Literal>>, anyhow::Error> {
    // all non-dominated solutions
    let mut results = Vec::new();
    loop {
        // get the next solution
        let solutions = match solver {
            SolverFamily::Sat => {
                get_solutions_no_dominance(Sat::default(), model.clone(), 1, solver_input_file)?
            }
            SolverFamily::Minion => {
                get_solutions_no_dominance(Minion::default(), model.clone(), 1, solver_input_file)?
            }
        };

        // no more solutions
        let Some(solution) = solutions.first() else {
            break;
        };

        // add to results
        results.extend(solutions.clone());

        // create and apply new blocking constraints
        model.add_constraints(crate_blocking_constraint_from_solution(
            &model, solution, dom_rel,
        ));
    }

    // vector constaining non-dominated solutions
    let mut final_results = Vec::new();

    // iterate over all found solutions and filter out those that are dominated by others
    for sol in results.iter() {
        let mut model_copy = model.clone();

        // remove blocking constraint created by the current solution
        model_copy.remove_constraints(crate_blocking_constraint_from_solution(
            &model_copy,
            sol,
            dom_rel,
        ));

        // add constraints for current solution (gives the variables fixed values)
        for (name, value) in sol.iter() {
            let expr = Expression::Atomic(
                Metadata::new(),
                Atom::Reference(DeclarationPtr::new(
                    name.clone(),
                    DeclarationKind::ValueLetting(Expression::Atomic(
                        Metadata::new(),
                        Atom::Literal(value.clone()),
                    )),
                )),
            );
            let val = Expression::Atomic(Metadata::new(), Atom::Literal(value.clone()));
            let eq = Expression::Eq(Metadata::new(), Moo::new(expr), Moo::new(val));
            model_copy.add_constraint(eq.clone());
        }

        // check if the solution is still valid
        let sols = match solver {
            SolverFamily::Sat => {
                get_solutions_no_dominance(Sat::default(), model_copy, -1, solver_input_file)?
            }
            SolverFamily::Minion => {
                get_solutions_no_dominance(Minion::default(), model_copy, -1, solver_input_file)?
            }
        };

        if !sols.is_empty() {
            final_results.push(sol.clone());
        }
    }
    Ok(final_results)
}

pub fn crate_blocking_constraint_from_solution(
    model: &Model,
    solution: &BTreeMap<Name, Literal>,
    dom_rel: &Expression,
) -> Vec<Expression> {
    use uniplate::Uniplate;

    // get blocking constraint expression
    let raw_blocking_constraint =
        dom_rel.rewrite(&|e| sub_in_solution_into_dominance_expr(&e, solution));

    let mut model_copy = model.clone();
    model_copy.remove_constraints(model_copy.as_submodel().constraints().clone());
    model_copy.add_constraint(raw_blocking_constraint);

    // rewrite model
    let rule_sets = model.context.read().unwrap().rule_sets.clone();
    let rewritten = rewrite_naive(&model_copy, &rule_sets, false, false);

    rewritten
        .expect("Should be able to rewrite the model")
        .as_submodel()
        .constraints()
        .clone()
}

pub fn sub_in_solution_into_dominance_expr(
    expr: &Expression,
    solution: &BTreeMap<Name, Literal>,
) -> Option<Expression> {
    use Expression::{Atomic, FromSolution};

    match expr {
        FromSolution(_, name_expr) => {
            if let Atomic(_, Atom::Reference(ptr)) = &**name_expr {
                let var_name = ptr.name();
                solution
                    .get(&var_name)
                    .map(|value| Atomic(Metadata::new(), Atom::Literal(value.clone())))
            } else {
                None
            }
        }

        _ => Some(expr.clone()),
    }
}

pub fn get_solutions_no_dominance(
    solver_adaptor: impl SolverAdaptor,
    model: Model,
    num_sols: i32,
    solver_input_file: &Option<PathBuf>,
) -> Result<Vec<BTreeMap<Name, Literal>>, anyhow::Error> {
    let adaptor_name = solver_adaptor.get_name().unwrap_or("UNKNOWN".into());
    let solver = Solver::new(solver_adaptor);

    eprintln!("Building {adaptor_name} model...");

    // Create for later since we consume the model when loading it
    let symbols_rc = Rc::clone(model.as_submodel().symbols_ptr_unchecked());

    let solver = solver.load_model(model)?;

    if let Some(solver_input_file) = solver_input_file {
        eprintln!(
            "Writing solver input file to {}",
            solver_input_file.display()
        );
        let mut file = std::fs::File::create(solver_input_file)?;
        solver.write_solver_input_file(&mut file)?;
    }

    eprintln!("Running {adaptor_name}...");

    // Create two arcs, one to pass into the solver callback, one to get solutions out later
    let all_solutions_ref = Arc::new(Mutex::<Vec<BTreeMap<Name, Literal>>>::new(vec![]));
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
    let mut sols_guard = (*all_solutions_ref).lock().unwrap();
    let sols = &mut *sols_guard;
    let symbols = symbols_rc.borrow();

    // Get the representations for each variable by name, since some variables are
    // divided into multiple auxiliary variables(see crate::representation::Representation)
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
        // Get the value of complex variables using their auxiliary variables
        for (name, representation) in representations.iter() {
            let value = representation.value_up(sol).unwrap();
            sol.insert(name.clone(), value);
        }

        // Remove auxiliary variables since we've found the value of the
        // variable they represent
        *sol = sol
            .clone()
            .into_iter()
            .filter(|(name, _)| !matches!(name, Name::Represented(_)))
            .collect();
    }

    sols.retain(|x| !x.is_empty());
    Ok(sols.clone())
}

#[allow(clippy::unwrap_used)]
pub fn get_solutions_from_conjure(
    essence_file: &str,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Vec<BTreeMap<Name, Literal>>, anyhow::Error> {
    let tmp_dir = tempdir()?;

    let mut cmd = std::process::Command::new("conjure");
    let output = cmd
        .arg("solve")
        .arg("--number-of-solutions=all")
        .arg("--copy-solutions=no")
        .arg("-o")
        .arg(tmp_dir.path())
        .arg(essence_file)
        .output()?;

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
            for (name, decl) in model.as_submodel().symbols().clone().into_iter() {
                match &decl.kind() as &DeclarationKind {
                    conjure_cp::ast::DeclarationKind::ValueLetting(expression) => {
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

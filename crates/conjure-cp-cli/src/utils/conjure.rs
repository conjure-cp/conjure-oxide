use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::string::ToString;
use std::sync::{Arc, Mutex, RwLock};

use conjure_cp::ast::{Atom, DeclarationKind, Expression, GroundDomain, Literal, Metadata, Name};
use conjure_cp::bug;
use conjure_cp::context::Context;
use conjure_cp::settings::{configured_rule_trace_enabled, set_rule_trace_enabled};

use serde_json::{Map, Value as JsonValue};

use itertools::Itertools as _;
use tempfile::tempdir;

use crate::utils::json::sort_json_object;
use conjure_cp::Model;
use conjure_cp::parse::tree_sitter::parse_essence_file;
use conjure_cp::solver::Solver;

use glob::glob;

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use uniplate::Uniplate;

#[derive(Clone, Copy, Debug, Default)]
pub struct ConjureRunTimings {
    pub translation_time_s: f64,
    pub conjure_translation_time_s: f64,
    pub savilerow_translation_time_s: f64,
    pub solve_time_s: f64,
}

#[derive(Debug)]
pub struct ConjureSolutions {
    pub solutions: Vec<BTreeMap<Name, Literal>>,
    pub timings: Option<ConjureRunTimings>,
}

/// Coerces a literal into the type expected by a reference domain when possible.
///
/// This is currently used to turn `0`/`1` solver outputs back into boolean
/// literals before substituting them into dominance expressions.
fn literal_for_reference_domain(
    reference_domain: Option<GroundDomain>,
    value: &Literal,
) -> Option<Literal> {
    if matches!(reference_domain, Some(GroundDomain::Bool)) {
        return match value {
            Literal::Bool(x) => Some(Literal::Bool(*x)),
            Literal::Int(1) => Some(Literal::Bool(true)),
            Literal::Int(0) => Some(Literal::Bool(false)),
            _ => None,
        };
    }

    Some(value.clone())
}

/// Replaces `fromSolution(x)` occurrences with the value of `x` from a previous solution.
fn substitute_from_solution(
    expr: &Expression,
    previous_solution: &BTreeMap<Name, Literal>,
) -> Option<Expression> {
    match expr {
        Expression::FromSolution(_, atom_expr) => {
            let Atom::Reference(reference) = atom_expr.as_ref() else {
                return Some(expr.clone());
            };

            let name = reference.name();
            let value = previous_solution.get(&name)?;
            let reference_domain = reference.resolved_domain().map(|x| x.as_ref().clone());
            let value = literal_for_reference_domain(reference_domain, value)?;
            Some(Expression::Atomic(Metadata::new(), Atom::Literal(value)))
        }
        _ => Some(expr.clone()),
    }
}

/// Replaces direct variable references with values from the candidate solution.
fn substitute_current_solution_refs(
    expr: &Expression,
    candidate_solution: &BTreeMap<Name, Literal>,
) -> Option<Expression> {
    match expr {
        Expression::Atomic(_, Atom::Reference(reference)) => {
            let name = reference.name();
            let value = candidate_solution.get(&name)?;
            let reference_domain = reference.resolved_domain().map(|x| x.as_ref().clone());
            let value = literal_for_reference_domain(reference_domain, value)?;
            Some(Expression::Atomic(Metadata::new(), Atom::Literal(value)))
        }
        _ => Some(expr.clone()),
    }
}

/// Evaluates whether `candidate_solution` dominates `previous_solution`.
///
/// The dominance expression is instantiated with values from both solutions and
/// then constant-folded to a boolean result.
fn does_solution_dominate(
    dominance_expression: &Expression,
    candidate_solution: &BTreeMap<Name, Literal>,
    previous_solution: &BTreeMap<Name, Literal>,
) -> bool {
    let expr = dominance_expression
        .rewrite(&|e| substitute_from_solution(&e, previous_solution))
        .rewrite(&|e| substitute_current_solution_refs(&e, candidate_solution));

    matches!(
        conjure_cp::ast::eval::eval_constant(&expr),
        Some(Literal::Bool(true))
    )
}

/// Removes solutions that are dominated by another solution in the result set.
fn retroactively_prune_dominated(
    solutions: Vec<BTreeMap<Name, Literal>>,
    dominance_expression: &Expression,
) -> Vec<BTreeMap<Name, Literal>> {
    solutions
        .iter()
        .enumerate()
        .filter_map(|(i, solution)| {
            let dominated = solutions.iter().enumerate().any(|(j, candidate)| {
                i != j && does_solution_dominate(dominance_expression, candidate, solution)
            });

            if dominated {
                None
            } else {
                Some(solution.clone())
            }
        })
        .collect()
}

pub fn get_solutions(
    solver: Solver,
    model: Model,
    num_sols: i32,
    solver_input_file: &Option<PathBuf>,
    rule_trace_cdp: bool,
) -> Result<Vec<BTreeMap<Name, Literal>>, anyhow::Error> {
    set_rule_trace_enabled(rule_trace_cdp && configured_rule_trace_enabled());

    let dominance_expression = model.dominance.as_ref().map(|expr| match expr {
        Expression::DominanceRelation(_, inner) => inner.as_ref().clone(),
        _ => expr.clone(),
    });

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
    let all_solutions_ref = Arc::new(Mutex::<Vec<BTreeMap<Name, Literal>>>::new(vec![]));
    let all_solutions_ref_2 = all_solutions_ref.clone();

    let solver = if num_sols > 0 {
        // Get num_sols solutions
        let sols_left = Mutex::new(num_sols);

        solver
            .solve(Box::new(move |sols| {
                let mut all_solutions = (*all_solutions_ref_2).lock().unwrap();
                (*all_solutions).push(sols.into_iter().collect());
                let mut sols_left = sols_left.lock().unwrap();
                *sols_left -= 1;

                *sols_left != 0
            }))
            .map_err(|err| anyhow::anyhow!("solver failed while collecting solutions: {err}"))?
    } else {
        // Get all solutions
        solver
            .solve(Box::new(move |sols| {
                let mut all_solutions = (*all_solutions_ref_2).lock().unwrap();
                (*all_solutions).push(sols.into_iter().collect());
                true
            }))
            .map_err(|err| anyhow::anyhow!("solver failed while collecting solutions: {err}"))?
    };

    solver.save_stats_to_context();

    // Get the collections of solutions and model symbols
    #[allow(clippy::unwrap_used)]
    let mut sols_guard = (*all_solutions_ref).lock().unwrap();
    let sols = &mut *sols_guard;
    let symbols = symbols_ptr.read();

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
            let value = representation.value_up(sol).map_err(|err| {
                anyhow::anyhow!(
                    "failed to reconstruct value for variable {name} from solver solution: {err}"
                )
            })?;
            sol.insert(name.clone(), value);
        }

        // Remove auxiliary variables since we've found the value of the
        // variable they represent
        *sol = sol
            .clone()
            .into_iter()
            .filter(|(name, _)| {
                !matches!(name, Name::Represented(_)) && !matches!(name, Name::Machine(_))
            })
            .collect();
    }

    sols.retain(|x| !x.is_empty());
    if let Some(dominance_expression) = dominance_expression.as_ref() {
        let pre_prune_len = sols.len();
        let pruned = retroactively_prune_dominated(sols.clone(), dominance_expression);
        let post_prune_len = pruned.len();

        eprintln!("Dominance pruning retained {post_prune_len} of {pre_prune_len} solutions.");

        *sols = pruned;
    }

    Ok(sols.clone())
}

#[derive(Clone, Debug, Default)]
pub struct ConjureSolveCaptureOptions {
    /// When set, `conjure solve -o` writes models and Minion files here instead of a temp dir.
    pub artifact_dir: Option<PathBuf>,
    /// Passed to `conjure solve --savilerow-options` (e.g. `-O0`).
    pub savilerow_options: Option<String>,
}

#[allow(clippy::unwrap_used)]
pub fn get_solutions_from_conjure(
    essence_file: &str,
    param_file: Option<&str>,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Vec<BTreeMap<Name, Literal>>, anyhow::Error> {
    Ok(
        get_solutions_from_conjure_with_stats(
            essence_file,
            param_file,
            context,
            0,
            ConjureSolveCaptureOptions::default(),
        )?
        .solutions,
    )
}

#[allow(clippy::unwrap_used)]
pub fn get_solutions_from_conjure_with_stats(
    essence_file: &str,
    param_file: Option<&str>,
    context: Arc<RwLock<Context<'static>>>,
    number_of_solutions: i32,
    capture_options: ConjureSolveCaptureOptions,
) -> Result<ConjureSolutions, anyhow::Error> {
    enum ConjureOutputDir {
        Temp(tempfile::TempDir),
        Fixed(PathBuf),
    }

    impl ConjureOutputDir {
        fn path(&self) -> &std::path::Path {
            match self {
                Self::Temp(dir) => dir.path(),
                Self::Fixed(path) => path,
            }
        }
    }

    let output_dir = match &capture_options.artifact_dir {
        Some(path) => {
            fs::create_dir_all(path)?;
            ConjureOutputDir::Fixed(path.clone())
        }
        None => ConjureOutputDir::Temp(tempdir()?),
    };

    let mut cmd = std::process::Command::new("conjure");
    let number_of_solutions_arg = if number_of_solutions == 0 {
        "all".to_string()
    } else {
        number_of_solutions.to_string()
    };

    cmd.arg("solve")
        .arg(format!("--number-of-solutions={number_of_solutions_arg}"))
        .arg("--copy-solutions=no")
        .arg("-o")
        .arg(output_dir.path());

    if let Some(options) = &capture_options.savilerow_options {
        cmd.arg(format!("--savilerow-options={options}"));
    }

    cmd.arg(essence_file);

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
        glob(&format!("{}/*.solution", output_dir.path().display()))?.collect();

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

    let timings = read_conjure_timings(output_dir.path())?;

    Ok(ConjureSolutions {
        solutions: solutions_set
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect(),
        timings,
    })
}

fn read_conjure_timings(
    path: &std::path::Path,
) -> Result<Option<ConjureRunTimings>, anyhow::Error> {
    let stats_files: Vec<_> = glob(&format!("{}/*.stats.json", path.display()))?.collect();
    if stats_files.is_empty() {
        return Ok(None);
    }

    let mut timings = ConjureRunTimings::default();
    for stats_file in stats_files {
        let stats_file = stats_file?;
        let stats: JsonValue = serde_json::from_str(&fs::read_to_string(&stats_file)?)?;
        let total_time = stats
            .get("totalTime")
            .and_then(JsonValue::as_f64)
            .unwrap_or_default();
        let savilerow_total_time = stats
            .pointer("/savilerowInfo/SavileRowTotalTime")
            .and_then(JsonValue::as_str)
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or_default();
        let solve_time = stats
            .pointer("/savilerowInfo/SolverTotalTime")
            .and_then(JsonValue::as_str)
            .and_then(|value| value.parse::<f64>().ok())
            .or_else(|| {
                stats
                    .pointer("/savilerowInfo/SolverSolveTime")
                    .and_then(JsonValue::as_str)
                    .and_then(|value| value.parse::<f64>().ok())
            })
            .unwrap_or_default();

        let conjure_translation_time = (total_time - savilerow_total_time).max(0.0);
        let savilerow_translation_time = (savilerow_total_time - solve_time).max(0.0);

        timings.conjure_translation_time_s += conjure_translation_time;
        timings.savilerow_translation_time_s += savilerow_translation_time;
        timings.translation_time_s += conjure_translation_time + savilerow_translation_time;
        timings.solve_time_s += solve_time;
    }

    Ok(Some(timings))
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

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{DeclarationPtr, Domain, Moo, Reference};

    #[test]
    fn retroactive_pruning_removes_dominated_prior_solution() {
        let x = Name::User("x".into());
        let x_ref = Expression::Atomic(
            Metadata::new(),
            Atom::Reference(Reference::new(DeclarationPtr::new_find(
                x.clone(),
                Domain::bool(),
            ))),
        );
        let dominance_expression = Expression::Imply(
            Metadata::new(),
            Moo::new(x_ref.clone()),
            Moo::new(Expression::FromSolution(
                Metadata::new(),
                Moo::new(Atom::Reference(Reference::new(DeclarationPtr::new_find(
                    x.clone(),
                    Domain::bool(),
                )))),
            )),
        );

        let mut sol_true = BTreeMap::new();
        sol_true.insert(x.clone(), Literal::Int(1));
        let mut sol_false = BTreeMap::new();
        sol_false.insert(x.clone(), Literal::Int(0));

        let pruned =
            retroactively_prune_dominated(vec![sol_true, sol_false.clone()], &dominance_expression);

        assert_eq!(pruned, vec![sol_false]);
    }
}

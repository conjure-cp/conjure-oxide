//! Collect solver results and reconstruct high-level Essence values.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::Model;
use crate::ast::categories::{Category, CategoryOf};
use crate::ast::{Atom, DeclarationPtr, Expression, GroundDomain, Literal, Metadata, Name};
use crate::bug_assert;
use crate::representation::util::try_up;
use crate::settings::{configured_rule_trace_enabled, rule_trace_enabled, set_rule_trace_enabled};
use crate::solver::{SearchStatus, Solver};
use uniplate::Uniplate;

struct RuleTraceGuard(bool);

impl Drop for RuleTraceGuard {
    fn drop(&mut self) {
        set_rule_trace_enabled(self.0);
    }
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
        crate::ast::eval::eval_constant(&expr),
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

fn validate_solution_collection_options(model: &Model, num_sols: i32) -> Result<(), anyhow::Error> {
    if model.objective.is_some() && num_sols != 1 {
        let got = if num_sols == 0 {
            "all".to_string()
        } else {
            num_sols.to_string()
        };
        return Err(anyhow::anyhow!(
            "optimisation problems require number-of-solutions=1 (got {got})"
        ));
    }
    Ok(())
}

/// Options for collecting high-level solutions from a rewritten model.
#[derive(Clone, Debug)]
pub struct SolutionOptions {
    /// Positive solution limit, or zero to enumerate all solutions.
    /// Optimisation requires a limit of one.
    pub number_of_solutions: i32,
    /// Retain every improving optimisation solution instead of only the optimum.
    pub keep_intermediate_solutions: bool,
    /// Write the solver's native input format to this path before searching.
    pub solver_input_file: Option<PathBuf>,
    /// Include solver-time constraint dominance rewrites in configured rule traces.
    pub rule_trace_cdp: bool,
}

impl Default for SolutionOptions {
    fn default() -> Self {
        Self {
            number_of_solutions: 1,
            keep_intermediate_solutions: false,
            solver_input_file: None,
            rule_trace_cdp: false,
        }
    }
}

/// Wall-clock phases of collecting solutions from a compiled model.
#[derive(Clone, Copy, Debug, Default)]
pub struct SolutionCollectionTimings {
    /// Time spent loading the rewritten model into the solver.
    pub backend_time_s: f64,
    /// Time inside the solver call, including solution callbacks and solver-time rewrites.
    pub search_time_s: f64,
    /// Time spent reconstructing values and pruning dominated solutions.
    pub solution_processing_time_s: f64,
}

/// Collect and reconstruct model solutions.
///
/// `num_sols` is a positive limit, or zero to enumerate all solutions. Optimisation
/// requires a limit of one and retains the optimum unless intermediate solutions are requested.
pub fn get_solutions(
    solver: Solver,
    model: Model,
    num_sols: i32,
    keep_intermediate_solutions: bool,
    solver_input_file: &Option<PathBuf>,
    rule_trace_cdp: bool,
) -> Result<Vec<BTreeMap<Name, Literal>>, anyhow::Error> {
    get_solutions_with_timings(
        solver,
        model,
        num_sols,
        keep_intermediate_solutions,
        solver_input_file,
        rule_trace_cdp,
    )
    .map(|(solutions, _)| solutions)
}

/// Collect solutions together with measured backend, search and reconstruction times.
pub fn get_solutions_with_timings(
    solver: Solver,
    model: Model,
    num_sols: i32,
    keep_intermediate_solutions: bool,
    solver_input_file: &Option<PathBuf>,
    rule_trace_cdp: bool,
) -> Result<(Vec<BTreeMap<Name, Literal>>, SolutionCollectionTimings), anyhow::Error> {
    let _rule_trace = RuleTraceGuard(rule_trace_enabled());
    set_rule_trace_enabled(rule_trace_cdp && configured_rule_trace_enabled());

    validate_solution_collection_options(&model, num_sols)?;

    let is_optimisation = model.objective.is_some();

    let dominance_expression = model.dominance.as_ref().map(|expr| match expr {
        Expression::DominanceRelation(_, inner) => inner.as_ref().clone(),
        _ => expr.clone(),
    });

    let adaptor_name = solver.get_name();

    tracing::info!(target: "conjure::stage", adaptor = adaptor_name, "building solver model");

    // Create for later since we consume the model when loading it
    let symbols_ptr = model.symbols_ptr_unchecked().clone();

    let backend_started_at = Instant::now();
    let solver = solver.load_model(model)?;
    let backend_time_s = backend_started_at.elapsed().as_secs_f64();

    if let Some(solver_input_file) = solver_input_file {
        tracing::info!(target: "conjure::stage",
            "Writing solver input file to {}",
            solver_input_file.display()
        );
        let file = Box::new(std::fs::File::create(solver_input_file)?);
        solver.write_solver_input_file(&mut (file as Box<dyn std::io::Write>))?;
    }

    tracing::info!(target: "conjure::stage", adaptor = adaptor_name, "running solver");

    // Create two arcs, one to pass into the solver callback, one to get solutions out later
    let all_solutions_ref = Arc::new(Mutex::<Vec<BTreeMap<Name, Literal>>>::new(vec![]));
    let all_solutions_ref_2 = all_solutions_ref.clone();

    let search_started_at = Instant::now();
    let solver = if is_optimisation {
        solver
            .solve(Box::new(move |sols| {
                let mut all_solutions = (*all_solutions_ref_2).lock().unwrap();
                let solution = sols.into_iter().collect();
                if keep_intermediate_solutions {
                    all_solutions.push(solution);
                } else {
                    all_solutions.clear();
                    all_solutions.push(solution);
                }
                true
            }))
            .map_err(|err| anyhow::anyhow!("solver failed while collecting solutions: {err}"))?
    } else if num_sols > 0 {
        // Get num_sols solutions
        let sols_left = Mutex::new(num_sols);

        solver
            .solve(Box::new(move |sols| {
                let mut all_solutions = (*all_solutions_ref_2).lock().unwrap();
                all_solutions.push(sols.into_iter().collect());
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
                all_solutions.push(sols.into_iter().collect());
                true
            }))
            .map_err(|err| anyhow::anyhow!("solver failed while collecting solutions: {err}"))?
    };

    let search_time_s = search_started_at.elapsed().as_secs_f64();
    let processing_started_at = Instant::now();
    let search_status = solver.search_status();
    solver.save_stats_to_context();

    // Get the collections of solutions and model symbols
    #[allow(clippy::unwrap_used)]
    let mut sols_guard = (*all_solutions_ref).lock().unwrap();
    let sols = &mut *sols_guard;

    // Stopping after a requested finite number of satisfaction solutions is intentional. Every
    // other incomplete search -- notably a solver timeout or interrupt while proving UNSAT -- must
    // not be accepted as a complete (possibly empty) solution set.
    let requested_limit_reached = !is_optimisation
        && num_sols > 0
        && sols.len() >= usize::try_from(num_sols).unwrap_or(usize::MAX);
    if let SearchStatus::Incomplete(reason) = search_status
        && !requested_limit_reached
    {
        return Err(anyhow::anyhow!("solver search incomplete: {reason:?}"));
    }

    let symbols = symbols_ptr.read();

    // Get the representations for each variable by name, since some variables are
    // divided into multiple auxiliary variables(see crate::representation::Representation)
    let names = symbols.clone().into_iter().map(|x| x.0).collect::<Vec<_>>();
    let representations = names
        .into_iter()
        .filter_map(|x| symbols.representations_for(&x).map(|repr| (x, repr)))
        .filter_map(|(name, reprs)| {
            if reprs.is_empty() {
                return None;
            }
            bug_assert!(
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
        .collect::<Vec<_>>();

    let structured_declarations: Vec<DeclarationPtr> = symbols
        .iter_local()
        .filter(|(name, declaration)| {
            // `category_of` follows a value letting into its body, and from there into every
            // letting that body names. Those declarations have no representations, so skip them
            // before walking the chain.
            matches!(name, Name::User(_))
                && !declaration.reprs().is_empty()
                && declaration.category_of() >= Category::Decision
        })
        .map(|(_, declaration)| declaration.clone())
        .collect();

    for sol in sols.iter_mut() {
        // Get the value of complex variables using their auxiliary variables
        for (name, representation) in representations.iter() {
            if sol.contains_key(name) {
                continue;
            }

            let value = representation.value_up(sol).map_err(|err| {
                anyhow::anyhow!(
                    "failed to reconstruct value for variable {name} from solver solution: {err}"
                )
            })?;
            sol.insert(name.clone(), value);
        }

        let raw_assignment: HashMap<Name, Literal> = sol.clone().into_iter().collect();
        for declaration in &structured_declarations {
            let value = try_up(declaration.clone(), &raw_assignment).map_err(|err| {
                anyhow::anyhow!(
                    "failed to reconstruct value for variable {}: {err}",
                    declaration.name()
                )
            })?;
            sol.insert(declaration.name().clone(), value);
        }

        // Minion represents native booleans as integers; return the declared Essence type.
        for (name, value) in sol.iter_mut() {
            let is_bool = symbols
                .lookup(name)
                .and_then(|declaration| declaration.domain())
                .is_some_and(|domain| matches!(domain.as_ground(), Some(GroundDomain::Bool)));
            if is_bool {
                match value {
                    Literal::Int(0) => *value = Literal::Bool(false),
                    Literal::Int(1) => *value = Literal::Bool(true),
                    _ => {}
                }
            }
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

    if let Some(dominance_expression) = dominance_expression.as_ref() {
        let pre_prune_len = sols.len();
        let pruned = retroactively_prune_dominated(sols.clone(), dominance_expression);
        let post_prune_len = pruned.len();

        tracing::info!(
            target: "conjure::stage",
            retained = post_prune_len,
            considered = pre_prune_len,
            "pruned dominated solutions"
        );

        *sols = pruned;
    }

    let solutions = sols.clone();
    Ok((
        solutions,
        SolutionCollectionTimings {
            backend_time_s,
            search_time_s,
            solution_processing_time_s: processing_started_at.elapsed().as_secs_f64(),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Domain, Moo, Reference};

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
            Moo::new(x_ref),
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
        sol_false.insert(x, Literal::Int(0));

        let pruned =
            retroactively_prune_dominated(vec![sol_true, sol_false.clone()], &dominance_expression);

        assert_eq!(pruned, vec![sol_false]);
    }
}

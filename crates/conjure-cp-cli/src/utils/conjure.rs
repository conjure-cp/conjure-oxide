use std::collections::BTreeMap;
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

/// Coerces a literal into the type expected by a reference domain when possible.
///
/// This is currently used to turn `0`/`1` solver outputs back into boolean
/// literals before substituting them into dominance expressions.
fn coerce_literal_to_domain(domain: &GroundDomain, value: &Literal) -> Option<Literal> {
    match domain {
        GroundDomain::Bool => match value {
            Literal::Bool(x) => Some(Literal::Bool(*x)),
            Literal::Int(1) => Some(Literal::Bool(true)),
            Literal::Int(0) => Some(Literal::Bool(false)),
            _ => None,
        },
        GroundDomain::Matrix(_elem_domain, _idx_domains) => {
            fn coerce_matrix_with_index_shape(
                elem_domain: &GroundDomain,
                idx_domains: &[conjure_cp::ast::Moo<GroundDomain>],
                value: &Literal,
            ) -> Option<Literal> {
                let Literal::AbstractLiteral(conjure_cp::ast::AbstractLiteral::Matrix(items, _)) =
                    value
                else {
                    return None;
                };
                let (head_idx, tail_idx) = idx_domains.split_first()?;

                let coerced_items = if tail_idx.is_empty() {
                    items
                        .iter()
                        .map(|item| coerce_literal_to_domain(elem_domain, item))
                        .collect::<Option<Vec<_>>>()?
                } else {
                    items
                        .iter()
                        .map(|item| coerce_matrix_with_index_shape(elem_domain, tail_idx, item))
                        .collect::<Option<Vec<_>>>()?
                };

                Some(Literal::AbstractLiteral(conjure_cp::ast::AbstractLiteral::Matrix(
                    coerced_items,
                    head_idx.clone(),
                )))
            }

            let GroundDomain::Matrix(elem_domain, idx_domains) = domain else {
                return None;
            };

            coerce_matrix_with_index_shape(elem_domain.as_ref(), idx_domains.as_slice(), value)
        }
        _ => Some(value.clone()),
    }
}

fn literal_for_reference_domain(
    reference_domain: Option<GroundDomain>,
    value: &Literal,
) -> Option<Literal> {
    match reference_domain {
        Some(domain) => coerce_literal_to_domain(&domain, value),
        None => Some(value.clone()),
    }
}

fn literal_int_value(lit: &Literal) -> Option<i32> {
    match lit {
        Literal::Int(i) => Some(*i),
        _ => None,
    }
}

fn synthesize_matrix_to_atom_literal(
    base_name: &str,
    elem_domain: &GroundDomain,
    idx_domains: &[conjure_cp::ast::Moo<GroundDomain>],
    prefix: &mut Vec<i32>,
    solution: &BTreeMap<Name, Literal>,
) -> Option<Literal> {
    if idx_domains.is_empty() {
        let suffix = prefix.iter().map(i32::to_string).collect_vec().join("_");
        let key = Name::user(&format!("{base_name}_{suffix}"));
        return solution.get(&key).cloned();
    }

    let (head_idx, tail_idx) = idx_domains.split_first()?;
    let values = head_idx.values().ok()?;
    let mut items = Vec::new();

    for v in values {
        let i = literal_int_value(&v)?;
        prefix.push(i);
        let item = if tail_idx.is_empty() {
            let suffix = prefix.iter().map(i32::to_string).collect_vec().join("_");
            let key = Name::user(&format!("{base_name}_{suffix}"));
            solution.get(&key)?.clone()
        } else {
            synthesize_matrix_to_atom_literal(base_name, elem_domain, tail_idx, prefix, solution)?
        };
        prefix.pop();
        items.push(item);
    }

    let _ = elem_domain;
    Some(Literal::AbstractLiteral(
        conjure_cp::ast::AbstractLiteral::Matrix(items, head_idx.clone()),
    ))
}

fn lookup_solution_value(
    name: &Name,
    reference_domain: Option<&GroundDomain>,
    solution: &BTreeMap<Name, Literal>,
) -> Option<Literal> {
    if let Some(v) = solution.get(name) {
        return Some(v.clone());
    }

    match name {
        Name::Represented(fields) => {
            let (source_name, _, _) = fields.as_ref();
            lookup_solution_value(source_name, reference_domain, solution)
        }
        Name::WithRepresentation(source_name, _) => {
            lookup_solution_value(source_name, reference_domain, solution)
        }
        Name::User(s) => {
            let s = s.as_str();
            if s.contains("#matrix_to_atom")
                && let Some(GroundDomain::Matrix(elem_domain, idx_domains)) = reference_domain
            {
                let mut prefix = Vec::new();
                if let Some(v) = synthesize_matrix_to_atom_literal(
                    s,
                    elem_domain.as_ref(),
                    idx_domains.as_slice(),
                    &mut prefix,
                    solution,
                ) {
                    return Some(v);
                }
            }
            let (base, _) = s.split_once('#')?;
            solution.get(&Name::user(base)).cloned()
        }
        Name::Machine(_) => None,
    }
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
            let reference_domain = reference.resolved_domain().map(|x| x.as_ref().clone());
            let value = match lookup_solution_value(&name, reference_domain.as_ref(), previous_solution) {
                Some(v) => v,
                None => {
                    return Some(expr.clone());
                }
            };
            let value = match literal_for_reference_domain(reference_domain, &value) {
                Some(v) => v,
                None => {
                    return Some(expr.clone());
                }
            };
            Some(Expression::Atomic(Metadata::new(), Atom::Literal(value)))
        }
        _ => None,
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
            let reference_domain = reference.resolved_domain().map(|x| x.as_ref().clone());
            let value = lookup_solution_value(&name, reference_domain.as_ref(), candidate_solution)?;
            let value = literal_for_reference_domain(reference_domain, &value)?;
            Some(Expression::Atomic(Metadata::new(), Atom::Literal(value)))
        }
        _ => None,
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
    let mut expr = dominance_expression
        .rewrite(&|e| substitute_from_solution(&e, previous_solution))
        .rewrite(&|e| substitute_current_solution_refs(&e, candidate_solution));

    // Saturate constant-folding so quantified/indexed inner expressions reduce
    // before we decide top-level dominance.
    for _ in 0..16 {
        let next = expr.rewrite(&|e| {
            conjure_cp::ast::eval::eval_constant(&e)
                .map(|lit| Expression::Atomic(Metadata::new(), Atom::Literal(lit)))
        });
        if next == expr {
            break;
        }
        expr = next;
    }

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
    }

    sols.retain(|x| !x.is_empty());
    if let Some(dominance_expression) = dominance_expression.as_ref() {
        let pre_prune_len = sols.len();
        let pruned = retroactively_prune_dominated(sols.clone(), dominance_expression);
        let post_prune_len = pruned.len();

        eprintln!("Dominance pruning retained {post_prune_len} of {pre_prune_len} solutions.");

        *sols = pruned;
    }

    for sol in sols.iter_mut() {
        // Remove auxiliary variables since we've found the value of the
        // variables they represent.
        *sol = sol
            .clone()
            .into_iter()
            .filter(|(name, _)| {
                !matches!(name, Name::Represented(_)) && !matches!(name, Name::Machine(_))
            })
            .collect();
    }
    sols.retain(|x| !x.is_empty());

    Ok(sols.clone())
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

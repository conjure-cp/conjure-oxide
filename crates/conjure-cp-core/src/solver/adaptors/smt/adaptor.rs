use std::collections::{BTreeMap, HashMap};
use std::iter::FusedIterator;

use itertools::Itertools;
use uniplate::Uniplate;
use versions::Versioning;
use z3::{
    Config, PrepareSynchronized, SatResult, Solvable, Solver, Statistics, Translate, with_z3_config,
};

use super::convert_model::*;
use super::store::*;
use super::theories::*;

use crate::ast::{Atom, Expression, GroundDomain, Literal, Metadata, Moo, Name};
use crate::rule_engine::rewrite_model_with_configured_rewriter;
use crate::settings::{Rewriter, current_rewriter, set_current_rewriter};
use crate::{Model, solver::*};

const MINIMUM_Z3_VERSION: &str = "4.8.12";

/// A [SolverAdaptor] for interacting with SMT solvers, specifically Z3.
pub struct Smt {
    __non_constructable: private::Internal,

    /// Initially maps variables to unknown constants.
    /// Also used to store their solved literal values.
    store: SymbolStore,

    /// Assertions are added to this solver instance when loading the model.
    solver_inst: Solver,

    solver_cfg: Config,

    theory_config: TheoryConfig,

    dominance_expression: Option<Expression>,
    dominance_model_template: Option<Model>,
}

impl private::Sealed for Smt {}

impl Default for Smt {
    fn default() -> Self {
        Smt {
            __non_constructable: private::Internal,
            store: SymbolStore::new(TheoryConfig::default()),
            solver_inst: Solver::new(),
            solver_cfg: Config::new(),
            theory_config: TheoryConfig::default(),
            dominance_expression: None,
            dominance_model_template: None,
        }
    }
}

impl Smt {
    /// Constructs a new adaptor using the given theories for representing the relevant constructs.
    pub fn new(timeout_msec: Option<u64>, theory_config: TheoryConfig) -> Self {
        let mut solver_cfg = Config::new();
        timeout_msec.inspect(|ms| solver_cfg.set_timeout_msec(*ms));

        Smt {
            theory_config,
            solver_cfg,
            store: SymbolStore::new(theory_config),
            ..Default::default()
        }
    }
}

fn sub_in_solution_into_dominance_expr(
    expr: &Expression,
    solution: &HashMap<Name, Literal>,
) -> Option<Expression> {
    match expr {
        Expression::FromSolution(_, atom_expr) => {
            if let Atom::Reference(reference) = atom_expr.as_ref() {
                let var_name = reference.name();
                let value = solution.get(&var_name)?;
                let value = if let Some(domain) = reference.resolved_domain() {
                    if domain.as_ref() == &GroundDomain::Bool {
                        match value {
                            Literal::Bool(x) => Literal::Bool(*x),
                            Literal::Int(1) => Literal::Bool(true),
                            Literal::Int(0) => Literal::Bool(false),
                            _ => return None,
                        }
                    } else {
                        value.clone()
                    }
                } else {
                    value.clone()
                };

                return Some(Expression::Atomic(Metadata::new(), Atom::Literal(value)));
            }
            Some(expr.clone())
        }
        _ => Some(expr.clone()),
    }
}

fn sub_in_solution_into_current_refs(
    expr: &Expression,
    solution: &HashMap<Name, Literal>,
) -> Option<Expression> {
    match expr {
        Expression::Atomic(_, Atom::Reference(reference)) => {
            let var_name = reference.name();
            let value = solution.get(&var_name)?;
            let value = if let Some(domain) = reference.resolved_domain() {
                if domain.as_ref() == &GroundDomain::Bool {
                    match value {
                        Literal::Bool(x) => Literal::Bool(*x),
                        Literal::Int(1) => Literal::Bool(true),
                        Literal::Int(0) => Literal::Bool(false),
                        _ => return None,
                    }
                } else {
                    value.clone()
                }
            } else {
                value.clone()
            };

            Some(Expression::Atomic(Metadata::new(), Atom::Literal(value)))
        }
        _ => Some(expr.clone()),
    }
}

fn swap_from_solution_to_current_ref(expr: &Expression) -> Option<Expression> {
    match expr {
        Expression::FromSolution(_, atom_expr) => Some(Expression::Atomic(
            Metadata::new(),
            atom_expr.as_ref().clone(),
        )),
        _ => Some(expr.clone()),
    }
}

fn add_represented_decision_values(solution: &mut HashMap<Name, Literal>, model: &Model) {
    let symbols = model.symbols().clone();
    let names = symbols.clone().into_iter().map(|x| x.0).collect_vec();
    let representations = names
        .into_iter()
        .filter_map(|name| {
            symbols
                .representations_for(&name)
                .map(|reprs| (name, reprs))
        })
        .filter_map(|(name, reprs)| {
            if reprs.is_empty() {
                return None;
            }
            if reprs.len() > 1 || reprs[0].len() != 1 {
                return None;
            }
            Some((name, reprs[0][0].clone()))
        })
        .collect_vec();

    if representations.is_empty() {
        return;
    }

    let mut solution_btree = solution
        .clone()
        .into_iter()
        .collect::<BTreeMap<Name, Literal>>();
    for (name, representation) in representations {
        let Ok(value) = representation.value_up(&solution_btree) else {
            continue;
        };
        solution.insert(name.clone(), value.clone());
        solution_btree.insert(name, value);
    }
}

fn extract_z3_version(full_version: &str) -> Result<&str, SolverError> {
    match full_version.strip_prefix("Z3 ") {
        Some(v) => v.split_whitespace().next().ok_or_else(|| {
            SolverError::Runtime(format!(
                "could not read Z3 runtime version from '{full_version}'"
            ))
        }),
        None => full_version.split_whitespace().next().ok_or_else(|| {
            SolverError::Runtime(format!(
                "could not read Z3 runtime version from '{full_version}'"
            ))
        }),
    }
}

fn ensure_supported_z3_runtime() -> Result<(), SolverError> {
    let full_version = z3::full_version();
    let runtime_version = extract_z3_version(full_version)?;
    let runtime_version = Versioning::new(runtime_version).ok_or_else(|| {
        SolverError::Runtime(format!(
            "could not parse Z3 runtime version from '{full_version}'"
        ))
    })?;
    let minimum_version =
        Versioning::new(MINIMUM_Z3_VERSION).expect("minimum Z3 version should be valid");

    if runtime_version < minimum_version {
        return Err(SolverError::Runtime(format!(
            "unsupported Z3 runtime version '{full_version}' (parsed as {runtime_version}); conjure-oxide requires Z3 >= {MINIMUM_Z3_VERSION}. This usually means an older system or precompiled Z3 was picked up at build time."
        )));
    }

    Ok(())
}

impl Smt {
    fn add_dominance_constraints_for_solution(
        dominance_expression: Option<&Expression>,
        dominance_model_template: Option<&Model>,
        solver: &mut Solver,
        store: &mut SymbolStore,
        theory_config: TheoryConfig,
        solution: &HashMap<Name, Literal>,
    ) -> Result<(), SolverError> {
        let Some(dominance_expression) = dominance_expression else {
            return Ok(());
        };

        let Some(model_template) = dominance_model_template else {
            return Ok(());
        };

        // Block future solutions dominated by the current solution:
        // assert NOT dominance(current_solution, future_solution).
        let rewritten_dominance = Expression::Not(
            Metadata::new(),
            Moo::new(
                dominance_expression
                    .rewrite(&|e| sub_in_solution_into_current_refs(&e, solution))
                    .rewrite(&|e| swap_from_solution_to_current_ref(&e)),
            ),
        );

        let mut dominance_model = model_template.clone();
        dominance_model.replace_constraints(vec![]);
        dominance_model.replace_clauses(vec![]);
        dominance_model.dominance = None;
        dominance_model.add_constraint(rewritten_dominance);

        let rule_sets = dominance_model.context.read().unwrap().rule_sets.clone();
        let rewritten =
            rewrite_model_with_configured_rewriter(dominance_model, &rule_sets, current_rewriter())
                .map_err(|e| {
                    SolverError::Runtime(format!(
                        "Failed to rewrite dominance constraint for SMT solving: {e}"
                    ))
                })?;

        load_model_impl(
            store,
            solver,
            &theory_config,
            &rewritten.symbols(),
            rewritten.constraints().as_slice(),
        )
    }
}

impl SolverAdaptor for Smt {
    fn solve(
        &mut self,
        callback: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        let solver_send = self.solver_inst.synchronized();
        let store_send = self.store.synchronized();
        let dominance_expression = self.dominance_expression.clone();
        let dominance_model_template = self.dominance_model_template.clone();
        let theory_config = self.theory_config;
        let mut stats: SolverStats = Default::default();

        // Apply config when getting solutions
        let (search_complete, final_z3_time) =
            with_z3_config(&self.solver_cfg, move || -> Result<_, SolverError> {
                let solver = solver_send.recover();
                let mut final_z3_time: Option<f64> = None;
                let mut found_solution = false;
                let mut hook_error: Option<SolverError> = None;
                let mut solutions = solver.into_solutions_with_statistics(
                    store_send.recover(),
                    true,
                    |solver, store, instance| {
                        let mut dominance_solution = match instance.as_literals_map() {
                            Ok(solution) => solution,
                            Err(err) => {
                                hook_error = Some(err);
                                return false;
                            }
                        };
                        if let Some(model_template) = dominance_model_template.as_ref() {
                            add_represented_decision_values(
                                &mut dominance_solution,
                                model_template,
                            );
                        }

                        match Smt::add_dominance_constraints_for_solution(
                            dominance_expression.as_ref(),
                            dominance_model_template.as_ref(),
                            solver,
                            store,
                            theory_config,
                            &dominance_solution,
                        ) {
                            Ok(()) => true,
                            Err(err) => {
                                hook_error = Some(err);
                                false
                            }
                        }
                    },
                );

                let _solution_count = solutions
                    .by_ref()
                    .take_while(|(instance, z3_stats)| {
                        found_solution = true;

                        let time = z3_stats.value("time");
                        if let Some(z3::StatisticsValue::Double(time)) = time {
                            final_z3_time = Some(time);
                        }

                        (callback)(instance.as_literals_map().unwrap())
                    })
                    .count();

                drop(solutions);
                if let Some(err) = hook_error {
                    return Err(err);
                }

                let search_complete = if found_solution {
                    SearchComplete::HasSolutions
                } else {
                    SearchComplete::NoSolutions
                };
                Ok((search_complete, final_z3_time))
            })?;

        if let Some(time) = final_z3_time {
            stats.solver_time_s = time;
        }

        Ok(SolveSuccess {
            stats,
            status: SearchStatus::Complete(search_complete),
        })
    }

    fn solve_mut(
        &mut self,
        callback: SolverMutCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(SolverError::OpNotImplemented("solve_mut".into()))
    }

    fn load_model(&mut self, model: Model, _: private::Internal) -> Result<(), SolverError> {
        // Fail fast if an older system or precompiled Z3 was linked in.
        ensure_supported_z3_runtime()?;
        self.dominance_expression = model.dominance.as_ref().map(|expr| match expr {
            Expression::DominanceRelation(_, inner) => inner.as_ref().clone(),
            _ => expr.clone(),
        });
        self.dominance_model_template = self.dominance_expression.as_ref().map(|_| model.clone());
        load_model_impl(
            &mut self.store,
            &mut self.solver_inst,
            &self.theory_config,
            &model.symbols(),
            model.constraints().as_slice(),
        )?;
        Ok(())
    }

    fn get_family(&self) -> SolverFamily {
        SolverFamily::Smt(self.theory_config)
    }

    fn get_name(&self) -> &'static str {
        "smt"
    }

    fn write_solver_input_file(
        &self,
        writer: &mut Box<dyn std::io::Write>,
    ) -> Result<(), std::io::Error> {
        let smt2 = self.solver_inst.to_smt2();
        writer.write(smt2.as_bytes()).map(|_| ())
    }
}

trait IntoSolutionsWithStatistics {
    fn into_solutions_with_statistics<T, F>(
        self,
        t: T,
        model_completion: bool,
        on_solution: F,
    ) -> SolverStatsIterator<T, F>
    where
        T: Solvable,
        F: FnMut(&mut Solver, &mut T, &T::ModelInstance) -> bool;
}

impl IntoSolutionsWithStatistics for Solver {
    fn into_solutions_with_statistics<T, F>(
        self,
        t: T,
        model_completion: bool,
        on_solution: F,
    ) -> SolverStatsIterator<T, F>
    where
        T: Solvable,
        F: FnMut(&mut Solver, &mut T, &T::ModelInstance) -> bool,
    {
        SolverStatsIterator {
            solver: self,
            ast: t,
            model_completion,
            on_solution,
            done: false,
        }
    }
}

struct SolverStatsIterator<T, F> {
    solver: Solver,
    ast: T,
    model_completion: bool,
    on_solution: F,
    done: bool,
}

impl<T, F> FusedIterator for SolverStatsIterator<T, F>
where
    T: Solvable,
    F: FnMut(&mut Solver, &mut T, &T::ModelInstance) -> bool,
{
}

impl<T, F> Iterator for SolverStatsIterator<T, F>
where
    T: Solvable,
    F: FnMut(&mut Solver, &mut T, &T::ModelInstance) -> bool,
{
    type Item = (T::ModelInstance, Statistics);

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        match self.solver.check() {
            SatResult::Sat => {
                let stats = self.solver.get_statistics();
                let model = self.solver.get_model()?;
                let instance = self.ast.read_from_model(&model, self.model_completion)?;
                let counterexample = self.ast.generate_constraint(&instance);

                if !(self.on_solution)(&mut self.solver, &mut self.ast, &instance) {
                    self.done = true;
                    return None;
                }

                self.solver.assert(counterexample);
                Some((instance, stats))
            }
            _ => {
                self.done = true;
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{DeclarationPtr, Domain, Moo, Reference};
    use crate::context::Context;

    #[test]
    fn from_solution_substitution_replaces_reference_with_literal() {
        let x = Name::User("x".into());
        let x_ref = DeclarationPtr::new_value_letting(
            x.clone(),
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(0))),
        );

        let expr = Expression::FromSolution(
            Metadata::new(),
            Moo::new(Atom::Reference(Reference::new(x_ref))),
        );
        let mut solution = HashMap::new();
        solution.insert(x, Literal::Int(7));

        let replaced = sub_in_solution_into_dominance_expr(&expr, &solution)
            .expect("FromSolution should be replaced when solution contains the variable");

        assert!(matches!(
            replaced,
            Expression::Atomic(_, Atom::Literal(Literal::Int(7)))
        ));
    }

    #[test]
    fn from_solution_substitution_returns_none_for_missing_solution_value() {
        let x = Name::User("x".into());
        let x_ref = DeclarationPtr::new_value_letting(
            x,
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(0))),
        );
        let expr = Expression::FromSolution(
            Metadata::new(),
            Moo::new(Atom::Reference(Reference::new(x_ref))),
        );
        let solution = HashMap::new();

        assert!(sub_in_solution_into_dominance_expr(&expr, &solution).is_none());
    }

    #[test]
    fn from_solution_substitution_coerces_ints_to_bool_for_bool_refs() {
        let x = Name::User("x".into());
        let x_ref = DeclarationPtr::new_find(x.clone(), Domain::bool());
        let expr = Expression::FromSolution(
            Metadata::new(),
            Moo::new(Atom::Reference(Reference::new(x_ref))),
        );
        let mut solution = HashMap::new();
        solution.insert(x, Literal::Int(1));

        let replaced = sub_in_solution_into_dominance_expr(&expr, &solution)
            .expect("FromSolution should be replaced when solution contains the variable");

        assert!(matches!(
            replaced,
            Expression::Atomic(_, Atom::Literal(Literal::Bool(true)))
        ));
    }

    #[test]
    fn adding_dominance_constraint_rules_out_solution_dominated_by_prior_solution() {
        let theory_config = TheoryConfig::default();
        let context = Context::new_ptr_empty(SolverFamily::Smt(theory_config));
        set_current_rewriter(Rewriter::Naive);

        let x = Name::User("x".into());
        let y = Name::User("y".into());

        let x_decl = DeclarationPtr::new_find(x.clone(), Domain::bool());
        let y_decl = DeclarationPtr::new_find(y.clone(), Domain::bool());
        let x_ref = Expression::Atomic(
            Metadata::new(),
            Atom::Reference(Reference::new(x_decl.clone())),
        );
        let y_ref = Expression::Atomic(
            Metadata::new(),
            Atom::Reference(Reference::new(y_decl.clone())),
        );

        let mut model = Model::new(context);
        model.add_symbol(x_decl);
        model.add_symbol(y_decl);
        model.dominance =
            Some(Expression::DominanceRelation(
                Metadata::new(),
                Moo::new(Expression::And(
                    Metadata::new(),
                    Moo::new(crate::matrix_expr![
                        Expression::Imply(
                            Metadata::new(),
                            Moo::new(x_ref.clone()),
                            Moo::new(Expression::FromSolution(
                                Metadata::new(),
                                Moo::new(Atom::Reference(Reference::new(
                                    DeclarationPtr::new_find(x.clone(), Domain::bool(),)
                                ))),
                            )),
                        ),
                        Expression::Imply(
                            Metadata::new(),
                            Moo::new(y_ref.clone()),
                            Moo::new(Expression::FromSolution(
                                Metadata::new(),
                                Moo::new(Atom::Reference(Reference::new(
                                    DeclarationPtr::new_find(y.clone(), Domain::bool(),)
                                ))),
                            )),
                        ),
                        Expression::Or(
                            Metadata::new(),
                            Moo::new(crate::matrix_expr![
                                Expression::And(
                                    Metadata::new(),
                                    Moo::new(crate::matrix_expr![
                                        Expression::Not(Metadata::new(), Moo::new(x_ref.clone())),
                                        Expression::FromSolution(
                                            Metadata::new(),
                                            Moo::new(Atom::Reference(Reference::new(
                                                DeclarationPtr::new_find(x.clone(), Domain::bool()),
                                            ))),
                                        ),
                                    ]),
                                ),
                                Expression::And(
                                    Metadata::new(),
                                    Moo::new(crate::matrix_expr![
                                        Expression::Not(Metadata::new(), Moo::new(y_ref.clone())),
                                        Expression::FromSolution(
                                            Metadata::new(),
                                            Moo::new(Atom::Reference(Reference::new(
                                                DeclarationPtr::new_find(y.clone(), Domain::bool()),
                                            ))),
                                        ),
                                    ]),
                                ),
                            ]),
                        ),
                    ]),
                )),
            ));

        let mut smt = Smt::new(None, theory_config);
        smt.load_model(model, private::Internal)
            .expect("SMT model should load");

        let mut prior_solution = HashMap::new();
        prior_solution.insert(x.clone(), Literal::Bool(true));
        prior_solution.insert(y.clone(), Literal::Bool(false));

        Smt::add_dominance_constraints_for_solution(
            smt.dominance_expression.as_ref(),
            smt.dominance_model_template.as_ref(),
            &mut smt.solver_inst,
            &mut smt.store,
            theory_config,
            &prior_solution,
        )
        .expect("dominance constraint should be assertable");

        let x_ast = smt
            .store
            .get(&x)
            .expect("x should exist in symbol store")
            .1
            .as_bool()
            .expect("x should be a bool");
        let y_ast = smt
            .store
            .get(&y)
            .expect("y should exist in symbol store")
            .1
            .as_bool()
            .expect("y should be a bool");

        smt.solver_inst.push();
        smt.solver_inst
            .assert(x_ast.eq(z3::ast::Bool::from_bool(true)));
        smt.solver_inst
            .assert(y_ast.eq(z3::ast::Bool::from_bool(true)));
        assert_eq!(smt.solver_inst.check(), SatResult::Unsat);
        smt.solver_inst.pop(1);

        smt.solver_inst.push();
        smt.solver_inst
            .assert(x_ast.eq(z3::ast::Bool::from_bool(false)));
        smt.solver_inst
            .assert(y_ast.eq(z3::ast::Bool::from_bool(false)));
        assert_eq!(smt.solver_inst.check(), SatResult::Sat);
        smt.solver_inst.pop(1);

        smt.solver_inst.push();
        smt.solver_inst
            .assert(x_ast.eq(z3::ast::Bool::from_bool(true)));
        smt.solver_inst
            .assert(y_ast.eq(z3::ast::Bool::from_bool(false)));
        assert_eq!(smt.solver_inst.check(), SatResult::Sat);
        smt.solver_inst.pop(1);
    }
}

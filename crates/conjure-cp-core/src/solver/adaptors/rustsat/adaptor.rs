use std::any::type_name;
use std::fmt::format;
use std::hash::Hash;
use std::iter::Inspect;
use std::ops::Deref;
use std::ptr::null;
use std::vec;

use clap::error;
use minion_sys::ast::{Model, Tuple};
use rustsat::encodings::am1::Def;
use rustsat::solvers::{ControlSignal, Solve, SolverResult, Terminate};
use rustsat::types::{Assignment, Clause, Lit, TernaryVal, Var as satVar};
use std::collections::{BTreeMap, HashMap};
use std::result::Result::Ok;
use std::time::Duration;
use tracing_subscriber::filter::DynFilterFn;
use ustr::Ustr;

use rustsat_cadical::CaDiCaL;

use crate::ast::pretty::pretty_vec;
use crate::ast::serde::HasId;
use crate::ast::{Atom, Expression, GroundDomain, Literal, Metadata, Moo, Name};
use crate::bug_assert;
use crate::representation::util::try_up;
use crate::rule_engine::{get_rule_sets_for_solver_family, rewrite_model_with_configured_rewriter};
use crate::settings::current_rewriter;
use crate::solver::SearchComplete::NoSolutions;
use crate::solver::adaptors::SolveTimeBudget;
use crate::solver::adaptors::rustsat::decisions::compile_decisions;
use crate::solver::{
    self, SearchStatus, SolveSuccess, SolverAdaptor, SolverCallback, SolverError, SolverFamily,
    SolverMutCallback, private,
};
use crate::stats::SolverStats;
use crate::{Model as ConjureModel, ast as conjure_ast, bug};
use crate::{into_matrix_expr, matrix_expr};

use rustsat::instances::{BasicVarManager, Cnf, ManageVars, SatInstance};

use thiserror::Error;
use uniplate::Uniplate;

use itertools::Itertools;
/// A [SolverAdaptor] for interacting with the SatSolver generic and the types thereof.
pub struct Sat {
    __non_constructable: private::Internal,
    solver_seed: u32,
    timeout: Option<Duration>,
    model_inst: Option<SatInstance>,
    var_map: Option<HashMap<Name, Lit>>,
    solver_inst: CaDiCaL<'static, 'static>,
    decision_refs: Option<Vec<Name>>,
    dominance_expression: Option<Expression>,
    dominance_model_template: Option<ConjureModel>,
}

impl private::Sealed for Sat {}

impl Default for Sat {
    fn default() -> Self {
        Sat {
            __non_constructable: private::Internal,
            solver_seed: 0,
            timeout: None,
            solver_inst: CaDiCaL::default(),
            var_map: None,
            model_inst: None,
            decision_refs: None,
            dominance_expression: None,
            dominance_model_template: None,
        }
    }
}

impl Sat {
    /// Sets the seed used by the SAT solver's random search behaviour.
    pub fn with_solver_seed(mut self, solver_seed: u32) -> Self {
        self.solver_seed = solver_seed;
        self
    }

    /// Sets one wall-clock budget shared by every SAT call made while enumerating solutions.
    pub fn with_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
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

fn rewrite_dominance_to_block_dominated_futures(
    dominance_expression: &Expression,
    solution: &HashMap<Name, Literal>,
) -> Expression {
    Expression::Not(
        Metadata::new(),
        Moo::new(
            dominance_expression
                .rewrite(&|e| sub_in_solution_into_current_refs(&e, solution))
                .rewrite(&|e| swap_from_solution_to_current_ref(&e)),
        ),
    )
}

fn add_represented_decision_values(solution: &mut HashMap<Name, Literal>, model: &ConjureModel) {
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

    for (_, declaration) in symbols.iter_local() {
        if declaration.reprs().is_empty() {
            continue;
        }
        if let Ok(value) = try_up(declaration.clone(), solution) {
            solution.insert(declaration.name().clone(), value);
        }
    }
}

fn get_ref_sols(
    find_refs: Vec<Name>,
    sol: Assignment,
    var_map: HashMap<Name, Lit>,
) -> HashMap<Name, Literal> {
    let mut solution: HashMap<Name, Literal> = HashMap::new();

    for reference in find_refs {
        // lit is `Nothing` for variables that don't exist. This should have thrown an error at parse-time.
        let lit: Lit = match var_map.get(&reference) {
            Some(a) => *a,
            None => bug!(
                "There should never be a non-just literal occurring here. Something is broken upstream."
            ),
        };

        solution.insert(
            reference,
            match sol[lit.var()] {
                TernaryVal::True => Literal::Int(1),
                TernaryVal::False => Literal::Int(0),
                TernaryVal::DontCare => Literal::Int(2),
            },
        );
    }

    solution
}

fn is_user_visible_solution_var(name: &Name) -> bool {
    !matches!(name, Name::Machine(_))
}

fn blocking_clause_for_solution(
    solution: &HashMap<Name, Literal>,
    var_map: &HashMap<Name, Lit>,
) -> Result<Clause, SolverError> {
    let mut clause = Clause::new();

    for (name, value) in solution {
        let lit = var_map.get(name).copied().ok_or_else(|| {
            SolverError::Runtime(format!(
                "Missing SAT variable for solution variable {name} when building blocking clause"
            ))
        })?;

        let blocking_lit = match value {
            Literal::Bool(true) | Literal::Int(1) => !lit,
            Literal::Bool(false) | Literal::Int(0) => lit,
            Literal::Int(2) => {
                return Err(SolverError::Runtime(format!(
                    "Cannot build blocking clause from dont-care assignment for {name}"
                )));
            }
            other => {
                return Err(SolverError::Runtime(format!(
                    "Cannot build SAT blocking clause from non-boolean value {other:?} for {name}"
                )));
            }
        };

        clause.add(blocking_lit);
    }

    Ok(clause)
}

impl Sat {
    fn add_dominance_constraints_for_solution(
        dominance_expression: Option<&Expression>,
        dominance_model_template: Option<&ConjureModel>,
        solver: &mut CaDiCaL<'static, 'static>,
        solution: &HashMap<Name, Literal>,
        var_map: &mut HashMap<Name, Lit>,
        next_free: &mut u32,
    ) -> Result<(), SolverError> {
        let Some(dominance_expression) = dominance_expression else {
            return Ok(());
        };

        let Some(model_template) = dominance_model_template else {
            return Ok(());
        };

        let rewritten_dominance =
            rewrite_dominance_to_block_dominated_futures(dominance_expression, solution);

        let mut dominance_model = model_template.clone();
        dominance_model.replace_constraints(vec![]);
        dominance_model.replace_sat_decisions(vec![]);
        dominance_model.dominance = None;
        dominance_model.add_constraint(rewritten_dominance);

        // Prefer the rule sets the model was built with, but fall back to resolving them for this
        // adaptor's own family: an embedder is not obliged to populate the context, and rewriting
        // a dominance constraint with no rules at all silently produces a model referring to
        // variables that representations have since replaced.
        let rule_sets = {
            let from_context = dominance_model.context.read().unwrap().rule_sets.clone();
            if from_context.is_empty() {
                get_rule_sets_for_solver_family(SolverFamily::Sat)
            } else {
                from_context
            }
        };
        let rewritten =
            rewrite_model_with_configured_rewriter(dominance_model, &rule_sets, current_rewriter())
                .map_err(|e| {
                    SolverError::Runtime(format!(
                        "Failed to rewrite dominance constraint into SAT decisions: {e}"
                    ))
                })?;

        let mut instance: SatInstance = SatInstance::new();
        instance
            .var_manager_mut()
            .increase_next_free(satVar::new(*next_free));
        compile_decisions(rewritten.sat_decisions(), &mut instance, var_map)?;
        for constraint in rewritten.constraints() {
            compile_decisions(
                &[crate::ast::SatEncodingDecision::Assert(constraint.clone())],
                &mut instance,
                var_map,
            )?;
        }
        let (cnf, manager): (Cnf, BasicVarManager) = instance.into_cnf();
        *next_free = manager.n_used();
        solver
            .add_cnf(cnf)
            .map_err(|e| SolverError::Runtime(format!("Failed adding dominance encoding: {e}")))?;

        Ok(())
    }
}

impl SolverAdaptor for Sat {
    fn solve(
        &mut self,
        callback: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        let budget = SolveTimeBudget::new(self.timeout);
        let timeout_enabled = self.timeout.is_some();
        let dominance_expression = self.dominance_expression.clone();
        let dominance_model_template = self.dominance_model_template.clone();
        let mut solver = &mut self.solver_inst;
        let solver_seed = i32::try_from(self.solver_seed).map_err(|_| {
            SolverError::Runtime(format!(
                "solver seed {} exceeds CaDiCaL's maximum supported value ({})",
                self.solver_seed,
                i32::MAX
            ))
        })?;
        solver.set_option("seed", solver_seed).map_err(|err| {
            SolverError::Runtime(format!("Failed setting CaDiCaL solver seed: {err}"))
        })?;
        let mut var_map = self.var_map.clone().ok_or_else(|| {
            SolverError::Runtime("Variable map is missing when retrieving solution".to_string())
        })?;

        let cnf: (Cnf, BasicVarManager) = self
            .model_inst
            .clone()
            .ok_or_else(|| SolverError::Runtime("Model instance is missing".to_string()))?
            .into_cnf();

        let mut next_free = cnf.1.n_used();
        solver.add_cnf(cnf.0).map_err(|e| {
            SolverError::Runtime(format!("Failed adding CNF to SAT solver before solve: {e}"))
        })?;

        if timeout_enabled {
            let terminator_budget = budget;
            solver.attach_terminator(move || {
                if terminator_budget.expired() {
                    ControlSignal::Terminate
                } else {
                    ControlSignal::Continue
                }
            });
        }

        let mut has_sol = false;
        loop {
            if budget.expired() {
                return Ok(SolveSuccess {
                    stats: SolverStats::default(),
                    status: SearchStatus::Incomplete(solver::SearchIncomplete::Timeout),
                });
            }

            let res = match solver.solve() {
                Ok(r) => r,
                Err(e) => {
                    return Err(SolverError::Runtime(format!(
                        "Solver encountered an error during solving: {}",
                        e
                    )));
                }
            };

            match res {
                SolverResult::Sat => {}
                SolverResult::Unsat => {
                    return Ok(SolveSuccess {
                        stats: SolverStats {
                            conjure_solver_wall_time_s: -1.0,
                            solver_family: Some(self.get_family()),
                            solver_adaptor: Some("SAT".to_string()),
                            ..Default::default()
                        },
                        status: if has_sol {
                            SearchStatus::Complete(solver::SearchComplete::HasSolutions)
                        } else {
                            SearchStatus::Complete(NoSolutions)
                        },
                    });
                }
                SolverResult::Interrupted => {
                    if timeout_enabled {
                        return Ok(SolveSuccess {
                            stats: SolverStats::default(),
                            status: SearchStatus::Incomplete(solver::SearchIncomplete::Timeout),
                        });
                    }
                    return Err(SolverError::Runtime(
                        "SAT solver was interrupted".to_string(),
                    ));
                }
            };

            let mut sol: Assignment = match solver.full_solution() {
                Ok(s) => s,
                Err(e) => {
                    return Err(SolverError::Runtime(format!(
                        "Solver encountered an error when retrieving solution: {}",
                        e
                    )));
                }
            };

            let find_refs = self.decision_refs.clone().ok_or_else(|| {
                SolverError::Runtime(
                    "Decision references are missing when retrieving solution".to_string(),
                )
            })?;

            for (name, lit) in &var_map {
                let inserter = sol.var_value(lit.var());
                sol.assign_var(lit.var(), inserter);
            }

            has_sol = true;
            let sol_old = get_ref_sols(find_refs.clone(), sol.clone(), var_map.clone());
            let full_assignment_solution = get_ref_sols(
                var_map.keys().cloned().collect(),
                sol.clone(),
                var_map.clone(),
            );

            tracing::info!("old solution {:#?}", sol_old);

            let solutions = enumerate_all_solutions(sol_old);

            for solution in solutions {
                tracing::info!("completed solution: {:#?}", solution);
                if budget.expired() {
                    return Ok(SolveSuccess {
                        stats: SolverStats::default(),
                        status: SearchStatus::Incomplete(solver::SearchIncomplete::Timeout),
                    });
                }
                if !callback(solution.clone()) {
                    // callback false
                    return Ok(SolveSuccess {
                        stats: SolverStats {
                            conjure_solver_wall_time_s: -1.0,
                            solver_family: Some(self.get_family()),
                            solver_adaptor: Some("SAT".to_string()),
                            ..Default::default()
                        },
                        status: SearchStatus::Incomplete(solver::SearchIncomplete::UserTerminated),
                    });
                }

                let mut dominance_solution = full_assignment_solution.clone();
                dominance_solution.extend(solution.clone());
                if let Some(model_template) = dominance_model_template.as_ref() {
                    add_represented_decision_values(&mut dominance_solution, model_template);
                }

                Sat::add_dominance_constraints_for_solution(
                    dominance_expression.as_ref(),
                    dominance_model_template.as_ref(),
                    solver,
                    &dominance_solution,
                    &mut var_map,
                    &mut next_free,
                )?;

                let blocking_cl = blocking_clause_for_solution(&solution, &var_map)?;
                tracing::info!("adding blocking clause for solution: {:#?}", solution);
                solver.add_clause(blocking_cl).map_err(|e| {
                    SolverError::Runtime(format!(
                        "Failed adding solution blocking clause to SAT solver: {e}"
                    ))
                })?;
                // Dominance can invalidate the remaining free completions of this assignment.
                if dominance_expression.is_some() {
                    break;
                }
            }
        }
    }

    fn solve_mut(
        &mut self,
        callback: SolverMutCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(SolverError::OpNotSupported("solve_mut".to_owned()))
    }

    fn load_model(&mut self, model: ConjureModel, _: private::Internal) -> Result<(), SolverError> {
        self.dominance_expression = model.dominance.as_ref().map(|expr| match expr {
            Expression::DominanceRelation(_, inner) => inner.as_ref().clone(),
            _ => expr.clone(),
        });
        self.dominance_model_template = self.dominance_expression.as_ref().map(|_| model.clone());

        if let Some(decision) = model.sat_encoding() {
            // Decision ASTs are terminal inputs. Never silently ignore residual constraints or
            // mix the two terminal decision payloads.
            if !model.constraints().is_empty()
                || !model.sat_decisions().is_empty()
                || !model.instantiation_conditions().is_empty()
                || model.dominance.is_some()
                || model.objective.is_some()
            {
                return Err(SolverError::ModelFeatureNotSupported(
                    "SAT decision AST cannot be mixed with residual constraints, semantic gate decisions, objectives, or dominance".into(),
                ));
            }
            let compiled = super::encoding_plan::CompiledBooleanDecision::compile(decision)
                .map_err(|error| SolverError::ModelFeatureNotSupported(error.to_string()))?;
            let (instance, literals) = compiled.into_parts();
            let symbols = model.symbols();
            let declarations: HashMap<_, _> = symbols
                .iter_local()
                .map(|(_, declaration)| (declaration.id(), declaration.clone()))
                .collect();
            let mut var_map = HashMap::new();
            let mut finds = Vec::new();
            for variable in decision.variable_ids() {
                let semantic = decision
                    .variable(variable)
                    .map_err(|error| SolverError::ModelFeatureNotSupported(error.to_string()))?;
                let declaration = declarations.get(&semantic.source).ok_or_else(|| {
                    SolverError::ModelFeatureNotSupported(format!(
                        "SAT decision variable `{}` has no declaration in the model",
                        semantic.name
                    ))
                })?;
                if !matches!(
                    &declaration.kind() as &crate::ast::DeclarationKind,
                    crate::ast::DeclarationKind::Find(_)
                        | crate::ast::DeclarationKind::FindAuxiliary(_)
                ) || declaration
                    .domain()
                    .and_then(|domain| domain.resolve().ok())
                    .is_none_or(|domain| *domain != GroundDomain::Bool)
                {
                    return Err(SolverError::ModelFeatureNotSupported(format!(
                        "SAT Boolean decision variable `{}` no longer has a Boolean find declaration",
                        semantic.name
                    )));
                }
                let name = declaration.name().clone();
                var_map.insert(name.clone(), literals[&variable]);
                if is_user_visible_solution_var(&name) {
                    finds.push(name);
                }
            }
            for (name, declaration) in symbols.iter_local() {
                if matches!(
                    &declaration.kind() as &crate::ast::DeclarationKind,
                    crate::ast::DeclarationKind::Find(_)
                        | crate::ast::DeclarationKind::FindAuxiliary(_)
                ) && !var_map.contains_key(name)
                {
                    return Err(SolverError::ModelFeatureNotSupported(format!(
                        "find `{name}` is missing from the SAT decision AST"
                    )));
                }
            }
            self.decision_refs = Some(finds);
            self.var_map = Some(var_map);
            self.model_inst = Some(instance);
            return Ok(());
        }

        // A residual false constraint makes the whole model unsatisfiable, even when previous
        // rewrites already emitted decisions. Preserve it before inspecting unencoded finds.
        if model
            .constraints()
            .iter()
            .any(|constraint| *constraint == false.into())
        {
            let mut inst = SatInstance::new();
            inst.add_clause(Clause::new());
            self.decision_refs = Some(Vec::new());
            self.var_map = Some(HashMap::new());
            self.model_inst = Some(inst);
            return Ok(());
        }

        let sym_tab = model.symbols().deref().clone();

        let mut finds: Vec<Name> = Vec::new();
        let mut var_map: HashMap<Name, Lit> = HashMap::new();

        for (name, decl) in sym_tab.into_iter_local() {
            if decl.as_find().is_none() {
                continue;
            }

            if !decl.reprs().is_empty() {
                continue;
            }

            let domain = decl
                .domain()
                .expect("Decision variable should have a domain");
            let domain = domain.as_ground().expect("Domain should be ground");

            // Everything reaching the solver must be Boolean by now. Integers are encoded into
            // Booleans by their representation, and a declaration that has one was skipped above.
            if domain != &GroundDomain::Bool {
                Err(SolverError::ModelInvalid(format!(
                    "Only Boolean Decision Variables supported, but '{name}' has domain {domain}"
                )))?;
            }
            // Only expose non-internal boolean variables in solver solutions. Machine names are
            // auxiliaries introduced during rewriting and can create huge powersets of don't-care
            // assignments without changing the semantic solution.
            if domain == &GroundDomain::Bool && is_user_visible_solution_var(&name) {
                finds.push(name);
            }
        }

        self.decision_refs = Some(finds.clone());

        let m_clone = model;

        // All constraints should have terminal encoding decisions.
        // the remaining constraint (if it exists) should just be a true/false expression
        let constraints = m_clone.constraints();
        bug_assert!(
            constraints.is_empty()
                || (constraints.len() == 1
                    && (constraints[0] == true.into() || constraints[0] == false.into())),
            "Un-encoded constraints in the model: {}",
            pretty_vec(constraints)
        );

        let decisions = m_clone.sat_decisions();

        let mut inst = SatInstance::new();
        finds.sort_by_key(ToString::to_string);
        for name in finds {
            var_map.insert(name, inst.new_lit());
        }
        compile_decisions(decisions, &mut inst, &mut var_map)?;

        self.var_map = Some(var_map);
        let cnf: (Cnf, BasicVarManager) = inst.clone().into_cnf();
        tracing::info!("CNF: {:?}", cnf.0);
        self.model_inst = Some(inst);

        Ok(())
    }

    fn init_solver(&mut self, _: private::Internal) {}

    fn get_family(&self) -> SolverFamily {
        SolverFamily::Sat
    }

    fn get_name(&self) -> &'static str {
        "sat"
    }

    fn write_solver_input_file(
        &self,
        writer: &mut Box<dyn std::io::Write>,
    ) -> Result<(), std::io::Error> {
        // TODO: add comments saying what conjure oxide variables each clause has
        // e.g.
        //      c y x z
        //        1 2 3
        //      c x -y
        //        1 -1
        // This will require handwriting a dimacs writer, but that should be easy. For now, just
        // let rustsat write the dimacs.

        let model = self.model_inst.clone().unwrap_or_else(|| {
            bug!("model should exist when we write the solver input file, as we should be in the LoadedModel state");
        });
        let (cnf, var_manager): (Cnf, BasicVarManager) = model.into_cnf();
        cnf.write_dimacs(writer, var_manager.n_used())
    }
}

/// Expand free Boolean assignments lazily so limits can stop exponential completion.
fn enumerate_all_solutions(
    mut solution: HashMap<Name, Literal>,
) -> impl Iterator<Item = HashMap<Name, Literal>> {
    let mut dont_cares: Vec<_> = solution
        .iter()
        .filter(|(_, value)| **value == Literal::Int(2))
        .map(|(name, _)| name.clone())
        .collect();
    dont_cares.sort_by_cached_key(ToString::to_string);
    for name in &dont_cares {
        solution.remove(name);
    }
    (0..dont_cares.len()).powerset().map(move |trues| {
        let mut completed = solution.clone();
        for (index, name) in dont_cares.iter().enumerate() {
            completed.insert(
                name.clone(),
                Literal::Int(i32::from(trues.contains(&index))),
            );
        }
        completed
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{DeclarationPtr, Domain, Moo, Reference};
    use crate::range;

    #[test]
    fn free_boolean_completions_are_unique_and_can_stop_early() {
        let free: HashMap<_, _> = (0..50)
            .map(|i| (Name::User(format!("free{i}").into()), Literal::Int(2)))
            .collect();
        assert_eq!(enumerate_all_solutions(free).take(100).count(), 100);
        let a = Name::User("a".into());
        let b = Name::User("b".into());
        let fixed = Name::User("fixed".into());
        let rows: Vec<_> = enumerate_all_solutions(HashMap::from([
            (a.clone(), Literal::Int(2)),
            (b.clone(), Literal::Int(2)),
            (fixed.clone(), Literal::Int(1)),
        ]))
        .collect();
        assert_eq!(rows.len(), 4);
        let choices: std::collections::HashSet<_> = rows
            .iter()
            .map(|row| {
                assert_eq!(row[&fixed], Literal::Int(1));
                (row[&a].clone(), row[&b].clone())
            })
            .collect();
        assert_eq!(choices.len(), 4);
        assert_eq!(enumerate_all_solutions(HashMap::new()).count(), 1);
    }

    #[test]
    fn free_boolean_completions_respect_added_dominance() {
        crate::settings::set_current_rewriter(crate::settings::Rewriter::Rewrite(
            crate::settings::RewriteConfig::baseline(),
        ));
        let mut model = ConjureModel::default();
        let p = DeclarationPtr::new_find(Name::User("p".into()), Domain::bool());
        model.add_symbol(p.clone()).unwrap();
        let reference: Expression = Reference::new(p.clone()).into();
        // False dominates true, so the first completion excludes the second.
        let dominance = Expression::And(
            Metadata::new(),
            Moo::new(into_matrix_expr!(vec![
                Expression::Not(Metadata::new(), Moo::new(reference.clone())),
                Expression::FromSolution(
                    Metadata::new(),
                    Moo::new(Atom::Reference(Reference::new(p)))
                ),
            ])),
        );
        model.dominance = Some(Expression::DominanceRelation(
            Metadata::new(),
            Moo::new(dominance),
        ));
        let mut sat = Sat::default();
        sat.load_model(model, private::Internal).unwrap();
        let rows = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let collected = rows.clone();
        sat.solve(
            Box::new(move |row| {
                collected.lock().unwrap().push(row);
                true
            }),
            private::Internal,
        )
        .unwrap();
        let rows = rows.lock().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][&Name::User("p".into())], Literal::Int(0));
    }

    use rustsat::types::Var as SatVar;

    fn boolean_decision_model() -> ConjureModel {
        use crate::ast::encoding_plan::*;
        let mut model = ConjureModel::default();
        let p = DeclarationPtr::new_find(Name::User("p".into()), Domain::bool());
        model.add_symbol(p.clone()).unwrap();
        let mut decision = EncodingDecision::default();
        let variable = decision.intern(&p, VariableOrigin::User).unwrap();
        let reference = decision
            .new_reference(variable, ReferenceContext::BooleanOperand)
            .unwrap();
        decision
            .add_plan(EncodingPlan {
                source_constraint: 0,
                kind: EncodingPlanKind::BooleanTseitin {
                    formula: BooleanFormula::Reference(reference),
                },
                requests: vec![RepresentationRequest {
                    reference,
                    kind: RepresentationKind::Boolean,
                }],
                provenance: PlanProvenance::ExplicitConfiguration,
            })
            .unwrap();
        model.set_sat_encoding(decision).unwrap();
        model
    }

    #[test]
    fn decision_ast_is_solved_without_model_cnf() {
        let model = boolean_decision_model();
        assert!(model.sat_decisions().is_empty());
        let serialized = serde_json::to_string(&crate::ast::SerdeModel::from(model)).unwrap();
        let imported: crate::ast::SerdeModel = serde_json::from_str(&serialized).unwrap();
        let mut sat = Sat::default();
        sat.load_model(
            imported.initialise(Default::default()).unwrap(),
            private::Internal,
        )
        .unwrap();
        let solutions = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let collected = solutions.clone();
        let result = sat
            .solve(
                Box::new(move |solution| {
                    collected.lock().unwrap().push(solution);
                    true
                }),
                private::Internal,
            )
            .unwrap();
        assert_eq!(
            result.status,
            SearchStatus::Complete(solver::SearchComplete::HasSolutions)
        );
        let solutions = solutions.lock().unwrap();
        assert_eq!(solutions.len(), 1);
        assert_eq!(solutions[0][&Name::User("p".into())], Literal::Int(1));
    }

    #[test]
    fn decision_ast_rejects_mixed_and_missing_semantics() {
        let mut mixed = boolean_decision_model();
        mixed.add_constraint(false.into());
        assert!(Sat::default().load_model(mixed, private::Internal).is_err());
        let mut missing = boolean_decision_model();
        missing
            .add_symbol(DeclarationPtr::new_find(
                Name::User("q".into()),
                Domain::bool(),
            ))
            .unwrap();
        assert!(
            Sat::default()
                .load_model(missing, private::Internal)
                .is_err()
        );
    }

    #[test]
    fn residual_false_is_unsatisfiable_even_with_existing_clauses_and_unencoded_finds() {
        let mut model = ConjureModel::new(Default::default());
        let boolean = model.symbols_mut().gen_find(&Domain::bool());
        model.symbols_mut().gen_find(&crate::domain_int!(0..5));
        model.add_sat_decision(crate::ast::SatEncodingDecision::Assert(
            Reference::new(boolean).into(),
        ));
        model.add_constraints(vec![false.into()]);
        let mut sat = Sat::default();
        sat.load_model(model, private::Internal).unwrap();
        let result = sat
            .solve(
                Box::new(|_| panic!("an inconsistent model has no solutions")),
                private::Internal,
            )
            .unwrap();
        assert_eq!(result.status, SearchStatus::Complete(NoSolutions));
    }

    #[test]
    fn zero_timeout_stops_before_first_sat_call() {
        let mut sat = Sat::default().with_timeout(Some(Duration::ZERO));
        sat.model_inst = Some(SatInstance::default());
        sat.var_map = Some(HashMap::new());
        sat.decision_refs = Some(Vec::new());

        let result = sat
            .solve(Box::new(|_| true), private::Internal)
            .expect("a timeout is a non-crashing incomplete search");

        assert_eq!(
            result.status,
            SearchStatus::Incomplete(solver::SearchIncomplete::Timeout)
        );
    }

    #[test]
    fn from_solution_substitution_replaces_reference_with_literal() {
        let x = Name::User(Ustr::from("x"));
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
        let x = Name::User(Ustr::from("x"));
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
        let x = Name::User(Ustr::from("x"));
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
}

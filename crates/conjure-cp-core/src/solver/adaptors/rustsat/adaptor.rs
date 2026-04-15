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
use rustsat::solvers::{Solve, SolverResult};
use rustsat::types::{Assignment, Clause, Lit, TernaryVal, Var as satVar};
use std::collections::{BTreeMap, HashMap};
use std::result::Result::Ok;
use tracing_subscriber::filter::DynFilterFn;
use ustr::Ustr;

use rustsat_cadical::CaDiCaL;

use crate::ast::pretty::pretty_vec;
use crate::ast::{Atom, Expression, GroundDomain, Literal, Metadata, Moo, Name};
use crate::rule_engine::rewrite_model_with_configured_rewriter;
use crate::settings::current_rewriter;
use crate::solver::SearchComplete::NoSolutions;
use crate::solver::adaptors::rustsat::convs::{cnf_clause_to_sat_clause, handle_cnf};
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
            solver_inst: CaDiCaL::default(),
            var_map: None,
            model_inst: None,
            decision_refs: None,
            dominance_expression: None,
            dominance_model_template: None,
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
        dominance_model.replace_clauses(vec![]);
        dominance_model.dominance = None;
        dominance_model.add_constraint(rewritten_dominance);

        let rule_sets = dominance_model.context.read().unwrap().rule_sets.clone();
        let rewritten =
            rewrite_model_with_configured_rewriter(dominance_model, &rule_sets, current_rewriter())
                .map_err(|e| {
                    SolverError::Runtime(format!(
                        "Failed to rewrite dominance constraint into CNF clauses: {e}"
                    ))
                })?;

        for clause in rewritten.clauses() {
            let mut missing_refs: Vec<Name> = Vec::new();
            let mut largest_new_var: Option<satVar> = None;
            for literal in clause.iter() {
                let maybe_name = match literal {
                    Expression::Atomic(_, Atom::Reference(reference)) => {
                        Some(reference.name().clone())
                    }
                    Expression::Not(_, inner) => {
                        if let Expression::Atomic(_, Atom::Reference(reference)) = inner.as_ref() {
                            Some(reference.name().clone())
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                if let Some(name) = maybe_name
                    && !var_map.contains_key(&name)
                {
                    missing_refs.push(name);
                }
            }

            if !missing_refs.is_empty() {
                missing_refs.sort_by_key(|name| name.to_string());
                missing_refs.dedup();

                for name in &missing_refs {
                    if var_map.contains_key(name) {
                        continue;
                    }
                    let next_idx = var_map
                        .values()
                        .map(|lit| lit.var().idx32())
                        .max()
                        .map(|idx| idx + 1)
                        .unwrap_or(0);
                    let new_var = satVar::new(next_idx);
                    let new_lit = new_var.pos_lit();
                    var_map.insert(name.clone(), new_lit);
                    largest_new_var = Some(new_var);
                }
            }

            if let Some(max_var) = largest_new_var {
                solver.reserve(max_var).map_err(|e| {
                    SolverError::Runtime(format!(
                        "Failed reserving SAT variable capacity up to {max_var} for dominance clauses: {e}"
                    ))
                })?;
            }

            if let Some(sat_clause) = cnf_clause_to_sat_clause(clause, var_map).map_err(|e| {
                SolverError::Runtime(format!(
                    "Failed converting dominance CNF clause to SAT clause. clause={clause:?}; error={e}"
                ))
            })? {
                solver.add_clause(sat_clause).map_err(|e| {
                    SolverError::Runtime(format!(
                        "Failed adding dominance clause to SAT solver: {e}"
                    ))
                })?;
            }
        }

        Ok(())
    }
}

impl SolverAdaptor for Sat {
    fn solve(
        &mut self,
        callback: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        let dominance_expression = self.dominance_expression.clone();
        let dominance_model_template = self.dominance_model_template.clone();
        let mut solver = &mut self.solver_inst;
        let mut var_map = self.var_map.clone().ok_or_else(|| {
            SolverError::Runtime("Variable map is missing when retrieving solution".to_string())
        })?;

        let cnf: (Cnf, BasicVarManager) = self
            .model_inst
            .clone()
            .ok_or_else(|| SolverError::Runtime("Model instance is missing".to_string()))?
            .into_cnf();

        solver.add_cnf(cnf.0).map_err(|e| {
            SolverError::Runtime(format!("Failed adding CNF to SAT solver before solve: {e}"))
        })?;

        let mut has_sol = false;
        loop {
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
                    return Err(SolverError::Runtime("!!Interrupted Solution!!".to_string()));
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

            tracing::info!("final solutions for run");
            tracing::info!("{:#?}", solutions);

            for solution in solutions {
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
                )?;

                let blocking_cl = blocking_clause_for_solution(&solution, &var_map)?;
                tracing::info!("adding blocking clause for solution: {:#?}", solution);
                solver.add_clause(blocking_cl).map_err(|e| {
                    SolverError::Runtime(format!(
                        "Failed adding solution blocking clause to SAT solver: {e}"
                    ))
                })?;
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

        let sym_tab = model.symbols().deref().clone();

        let mut finds: Vec<Name> = Vec::new();
        let mut var_map: HashMap<Name, Lit> = HashMap::new();

        for (name, decl) in sym_tab.clone().into_iter_local() {
            match decl.kind().clone() {
                conjure_ast::DeclarationKind::Find(_) => {
                    let domain = decl
                        .domain()
                        .expect("Decision variable should have a domain");
                    let domain = domain.as_ground().expect("Domain should be ground");

                    // only decision variables with boolean domains or representations using booleans are supported at this time
                    if (domain != &GroundDomain::Bool
                        && sym_tab
                            .get_representation(&name, &["sat_log_int"])
                            .is_none()
                        && sym_tab
                            .get_representation(&name, &["sat_direct_int"])
                            .is_none()
                        && sym_tab
                            .get_representation(&name, &["sat_order_int"])
                            .is_none())
                    {
                        Err(SolverError::ModelInvalid(
                            "Only Boolean Decision Variables supported".to_string(),
                        ))?;
                    }
                    // only boolean variables should be passed to the solver
                    if (domain == &GroundDomain::Bool && is_user_visible_solution_var(&name)) {
                        finds.push(name);
                    }
                }

                conjure_ast::DeclarationKind::ValueLetting(_expression, _)
                | conjure_ast::DeclarationKind::TemporaryValueLetting(_expression) => {}
                conjure_ast::DeclarationKind::DomainLetting(_moo) => {}
                conjure_ast::DeclarationKind::Given(_moo) => todo!(),
                conjure_ast::DeclarationKind::Quantified(_given_quantified) => todo!(),
                conjure_ast::DeclarationKind::RecordField(_moo) => todo!(),
            }
        }

        self.decision_refs = Some(finds.clone());

        let m_clone = model;

        // all constraints should be encoded as clauses
        // the remaining constraint (if it exists) should just be a true/false expression
        let constraints = m_clone.constraints();
        assert!(
            constraints.is_empty()
                || (constraints.len() == 1
                    && (constraints[0] == true.into() || constraints[0] == false.into())),
            "Un-encoded constraints in the model: {}",
            pretty_vec(constraints)
        );

        let clauses = m_clone.clauses();

        let inst: SatInstance = handle_cnf(clauses, &mut var_map, finds.clone());

        self.var_map = Some(var_map);
        let cnf: (Cnf, BasicVarManager) = inst.clone().into_cnf();
        tracing::info!("CNF: {:?}", cnf.0);
        self.model_inst = Some(inst);

        Ok(())
    }

    fn init_solver(&mut self, _: private::Internal) {}

    fn get_family(&self) -> SolverFamily {
        SolverFamily::Sat(crate::settings::SatEncoding::Log)
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

// Function that takes in solutions and returns updated solutions
// update consists of checking each assignment and calling enumerate_real if dont-care types exist
fn enumerate_all_solutions(solution: HashMap<Name, Literal>) -> Vec<HashMap<Name, Literal>> {
    tracing::info!("Enumerating");
    for (key, val) in solution.clone() {
        match val {
            Literal::Int(a) => {
                if (a == 2) {
                    return enumerate_solution(solution);
                } else {
                    continue;
                }
            }
            _ => continue,
        }
    }
    vec![solution]
}

// Function that takes in ONE solution and
// unfolds dont-care type ternaries into each possible type
// returns all possible 'real' solutions for this 'generated' solution
// a real solution is one with no variable assigned to Ternary::DontCare
fn enumerate_solution(solution: HashMap<Name, Literal>) -> Vec<HashMap<Name, Literal>> {
    tracing::info!("Enumerating: Real");
    let mut sols = Vec::new();
    let mut dont_cares = Vec::new();
    let mut solutions_inclusive = HashMap::new();

    for (key, val) in solution {
        let v = match val {
            Literal::Int(i) => i,
            _ => bug!("Only Integers expected at this time"),
        };
        if v == 2 {
            // anytime the value is 2 (dont-care in the ternary system used by rustsat), add the
            // key to a vector of dontcare values
            dont_cares.push(key);
        } else {
            // if the value is not a dont-care, then this (k, v) pair is usable in the final
            // solution, so just add it to the inclusive solution (another HashMap)
            solutions_inclusive.insert(key, val);
        }
    }

    let mut tdcs = Vec::new();

    tdcs.push(vec![]);
    for len in 1..(dont_cares.len()) {
        for combination in dont_cares.iter().combinations(len) {
            tdcs.push(combination);
        }
    }

    for trues in tdcs {
        let mut d = solutions_inclusive.clone();
        for key in dont_cares.clone() {
            if trues.contains(&&key) {
                d.insert(key, Literal::Int(1));
            } else {
                d.insert(key, Literal::Int(0));
            }
        }
        sols.push(d);
    }

    for i in dont_cares {
        solutions_inclusive.insert(i, Literal::Int(1));
    }

    sols.push(solutions_inclusive);
    sols
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{DeclarationPtr, Domain, Moo, Reference};
    use rustsat::types::Var as SatVar;

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

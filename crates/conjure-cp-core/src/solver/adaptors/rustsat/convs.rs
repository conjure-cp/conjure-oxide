use core::panic;
use std::{
    collections::HashMap,
    env::{Vars, vars},
    io::Lines,
};

use rustsat::{
    clause,
    instances::{BasicVarManager, Cnf, SatInstance},
    solvers::{Solve, SolverResult},
    types::{Clause, Lit, TernaryVal},
};

use thiserror::Error;

use anyhow::{Result, anyhow};

use crate::{
    ast::{Atom, CnfClause, Expression, Literal, Moo, Name},
    bug,
    solver::SolverError,
};

fn var_map_debug_summary(var_map: &HashMap<Name, Lit>) -> String {
    let mut names = var_map.keys().map(ToString::to_string).collect::<Vec<_>>();
    names.sort();
    let total = names.len();
    let preview = names.into_iter().take(25).collect::<Vec<_>>().join(", ");
    format!("known_vars_total={total}; known_vars_preview=[{preview}]")
}

pub fn handle_lit(
    l1: &Expression,
    vars_added: &mut HashMap<Name, Lit>,
    inst: &mut SatInstance,
) -> Lit {
    match l1 {
        // simple literal
        // TODO (ss504) check what can be done to avoid cloning
        Expression::Atomic(_, _) => handle_atom(l1.clone(), true, vars_added, inst),
        // not literal
        Expression::Not(_, _) => handle_not(l1, vars_added, inst),

        _ => bug!("Literal expected"),
    }
}

pub fn handle_not(
    expr: &Expression,
    vars_added: &mut HashMap<Name, Lit>,
    inst: &mut SatInstance,
) -> Lit {
    match expr {
        Expression::Not(_, ref_a) => {
            let ref_a = Moo::clone(ref_a);
            let a = Moo::unwrap_or_clone(ref_a);
            handle_atom(a, false, vars_added, inst)
        }
        _ => bug!("Not Expression Expected"),
    }
}

pub fn handle_atom(
    a: Expression,
    polarity: bool,
    vars_added: &mut HashMap<Name, Lit>,
    inst: &mut SatInstance,
) -> Lit {
    // polarity false for not
    match a {
        Expression::Atomic(_, atom) => match atom {
            conjure_cp_core::ast::Atom::Literal(literal) => {
                todo!("Not Sure if we are handling Lits as-is or not..")
            }
            conjure_cp_core::ast::Atom::Reference(reference) => match &*(reference.name()) {
                conjure_cp_core::ast::Name::User(_)
                | conjure_cp_core::ast::Name::Represented(_)
                | conjure_cp_core::ast::Name::Machine(_) => {
                    // TODO: Temp Clone
                    // let m = n.clone();
                    let lit_temp: Lit = fetch_lit(reference.name().clone(), vars_added, inst);
                    if polarity { lit_temp } else { !lit_temp }
                }
                _ => todo!("Not implemented yet"),
            },
        },
        _ => bug!("atomic expected"),
    }
}

pub fn fetch_lit(name: Name, vars_added: &mut HashMap<Name, Lit>, inst: &mut SatInstance) -> Lit {
    if !vars_added.contains_key(&name) {
        vars_added.insert(name.clone(), inst.new_lit());
    }
    *(vars_added
        .get(&name)
        .unwrap_or_else(|| bug!("Literal could not be fetched")))
}

pub fn handle_disjn(
    disjn: &CnfClause,
    vars_added: &mut HashMap<Name, Lit>,
    inst_in_use: &mut SatInstance,
) {
    let mut lits = Clause::new();

    for literal in disjn.iter() {
        let lit: Lit = handle_lit(literal, vars_added, inst_in_use);
        lits.add(lit);
    }

    inst_in_use.add_clause(lits);
}

pub fn handle_cnf(
    vec_cnf: &Vec<CnfClause>,
    vars_added: &mut HashMap<Name, Lit>,
    finds: Vec<Name>,
) -> SatInstance {
    let mut inst = SatInstance::new();

    tracing::info!("{:?} are all the decision vars found.", finds);

    for name in finds {
        vars_added.insert(name, inst.new_lit());
    }

    for disjn in vec_cnf {
        handle_disjn(disjn, vars_added, &mut inst);
    }

    inst
}

pub fn cnf_literal_to_sat_lit(
    literal: &Expression,
    var_map: &HashMap<Name, Lit>,
) -> Result<Option<Lit>, SolverError> {
    match literal {
        Expression::Atomic(_, Atom::Reference(reference)) => {
            let name = reference.name();
            let lit = var_map.get(&name).ok_or_else(|| {
                SolverError::Runtime(format!(
                    "CNF clause references unknown variable '{name}'. literal={literal:?}. {}",
                    var_map_debug_summary(var_map)
                ))
            })?;
            Ok(Some(*lit))
        }
        Expression::Not(_, inner) => {
            let Expression::Atomic(_, Atom::Reference(reference)) = inner.as_ref() else {
                return Err(SolverError::Runtime(
                    "CNF clause contains unsupported negated literal".to_string(),
                ));
            };
            let name = reference.name();
            let lit = var_map.get(&name).ok_or_else(|| {
                SolverError::Runtime(format!(
                    "CNF clause references unknown variable '{name}'. literal={literal:?}. {}",
                    var_map_debug_summary(var_map)
                ))
            })?;
            Ok(Some(!*lit))
        }
        Expression::Atomic(_, Atom::Literal(Literal::Bool(true))) => Ok(None),
        Expression::Atomic(_, Atom::Literal(Literal::Bool(false))) => Ok(None),
        _ => Err(SolverError::Runtime(format!(
            "CNF clause contains non-literal expression: {literal:?}"
        ))),
    }
}

pub fn cnf_clause_to_sat_clause(
    clause: &CnfClause,
    var_map: &HashMap<Name, Lit>,
) -> Result<Option<Clause>, SolverError> {
    let mut sat_clause = Clause::new();
    let mut has_false_only = false;

    for literal in clause.iter() {
        match literal {
            Expression::Atomic(_, Atom::Literal(Literal::Bool(true))) => {
                // Clause is tautologically true.
                return Ok(None);
            }
            Expression::Atomic(_, Atom::Literal(Literal::Bool(false))) => {
                has_false_only = true;
            }
            _ => {
                if let Some(lit) = cnf_literal_to_sat_lit(literal, var_map)? {
                    sat_clause.add(lit);
                    has_false_only = false;
                }
            }
        }
    }

    if sat_clause.iter().next().is_none() && !has_false_only {
        // Empty after simplification and no explicit false literal => tautology.
        return Ok(None);
    }

    Ok(Some(sat_clause))
}

// Error reserved for future use
// TODO: Integrate or remove
#[derive(Error, Debug)]
pub enum CNFError {
    #[error("Variable with name `{0}` not found")]
    VariableNameNotFound(String),

    #[error("Variable with name `{0}` not of right type")]
    BadVariableType(String),

    #[error("Unexpected Expression `{0}` inside Not(). Only Not(Reference) or Not(Not) allowed!")]
    UnexpectedExpressionInsideNot(Expression),

    #[error("Unexpected Expression `{0}` as literal. Only Not() or Reference() allowed!")]
    UnexpectedLiteralExpression(Expression),

    #[error("Unexpected Expression `{0}` inside And(). Only And(vec<Or>) allowed!")]
    UnexpectedExpressionInsideAnd(Expression),

    #[error("Unexpected Expression `{0}` inside Or(). Only Or(lit, lit) allowed!")]
    UnexpectedExpressionInsideOr(Expression),

    #[error("Unexpected Expression `{0}` found!")]
    UnexpectedExpression(Expression),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{DeclarationPtr, Metadata, Reference};

    fn mk_ref_expr(name: Name) -> Expression {
        let decl = DeclarationPtr::new_value_letting(
            name,
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(0))),
        );
        Expression::Atomic(Metadata::new(), Atom::Reference(Reference::new(decl)))
    }

    #[test]
    fn cnf_clause_to_sat_clause_maps_reference_literals() {
        let x = Name::User(ustr::Ustr::from("x"));
        let mut inst: SatInstance = SatInstance::new();
        let x_lit = inst.new_lit();
        let mut var_map = HashMap::new();
        var_map.insert(x.clone(), x_lit);

        let clause = CnfClause::new(vec![mk_ref_expr(x)]);
        let sat_clause = cnf_clause_to_sat_clause(&clause, &var_map)
            .expect("reference-only clause should convert")
            .expect("clause should not be dropped");

        let lits: Vec<Lit> = sat_clause.iter().copied().collect();
        assert_eq!(lits, vec![x_lit]);
    }

    #[test]
    fn cnf_clause_to_sat_clause_drops_true_tautology() {
        let var_map = HashMap::new();
        let clause = CnfClause::new(vec![Expression::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool(true)),
        )]);

        let sat_clause = cnf_clause_to_sat_clause(&clause, &var_map)
            .expect("true-only clause should be handled");

        assert!(sat_clause.is_none());
    }

    #[test]
    fn cnf_clause_to_sat_clause_keeps_false_only_clause_as_empty_clause() {
        let var_map = HashMap::new();
        let clause = CnfClause::new(vec![Expression::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool(false)),
        )]);

        let sat_clause = cnf_clause_to_sat_clause(&clause, &var_map)
            .expect("false-only clause should be handled")
            .expect("false-only clause should become an empty SAT clause");

        assert!(sat_clause.iter().next().is_none());
    }
}

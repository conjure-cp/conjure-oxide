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

use rustsat_minisat::core::Minisat;

use anyhow::{Result, anyhow};

use crate::{
    ast::{CnfClause, Expression, Moo, Name},
    bug,
    solver::Error,
};

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

        _ => todo!("Literal expected"),
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
        _ => todo!("Not Expression Expected"),
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
        _ => panic!("atomic expected"),
    }
}

pub fn fetch_lit(name: Name, vars_added: &mut HashMap<Name, Lit>, inst: &mut SatInstance) -> Lit {
    if !vars_added.contains_key(&name) {
        vars_added.insert(name.clone(), inst.new_lit());
    }
    *(vars_added.get(&name).unwrap())
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

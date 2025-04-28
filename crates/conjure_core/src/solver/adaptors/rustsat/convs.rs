use core::panic;
use std::{
    collections::HashMap,
    env::{vars, Vars},
    io::Lines,
};

use rustsat::{
    clause,
    instances::{BasicVarManager, Cnf, SatInstance},
    solvers::{Solve, SolverResult},
    types::{Clause, Lit, TernaryVal},
};

use rustsat_minisat::core::Minisat;

use anyhow::{anyhow, Result};

use crate::{ast::Expression, bug, solver::Error};

pub fn handle_lit(
    l1: &Expression,
    vars_added: &mut HashMap<String, Lit>,
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
    vars_added: &mut HashMap<String, Lit>,
    inst: &mut SatInstance,
) -> Lit {
    match expr {
        Expression::Not(_, ref_heap_a) => {
            // TODO (ss504) check what can be done to avoid cloning
            let a = ref_heap_a.clone();
            // and then unbox
            handle_atom(*a, false, vars_added, inst)
        }
        _ => todo!("Not Expression Expected"),
    }
}

pub fn handle_atom(
    a: Expression,
    polarity: bool,
    vars_added: &mut HashMap<String, Lit>,
    inst: &mut SatInstance,
) -> Lit {
    // polarity false for not
    match a {
        Expression::Atomic(_, atom) => match atom {
            conjure_core::ast::Atom::Literal(literal) => {
                todo!("Not Sure if we are handling Lits as-is or not..")
            }
            conjure_core::ast::Atom::Reference(name) => match name {
                conjure_core::ast::Name::UserName(n) => {
                    // TODO: Temp Clone
                    // let m = n.clone();
                    let lit_temp: Lit = fetch_lit(n, vars_added, inst);
                    if polarity {
                        lit_temp
                    } else {
                        !lit_temp
                    }
                }
                _ => {
                    todo!("Change Here for other types of vars")
                }
            },
        },
        _ => panic!("atomic expected"),
    }
}

pub fn fetch_lit(
    symbol: String,
    vars_added: &mut HashMap<String, Lit>,
    inst: &mut SatInstance,
) -> Lit {
    if !vars_added.contains_key(&symbol) {
        bug!("The code should never reach this point.");
    }
    *(vars_added.get(&symbol).unwrap())
}

pub fn handle_disjn(
    disjn: &Expression,
    vars_added: &mut HashMap<String, Lit>,
    inst_in_use: &mut SatInstance,
) {
    let cl: &Vec<Expression> = match disjn {
        Expression::Or(_, vec) => &vec.clone().unwrap_list().unwrap(),
        // Expression::F
        _ => bug!("This should always be either a Disjunction or a Constant; Found:{cl}"),
    };

    let mut clause: Clause = Clause::new();
    for lit in cl {
        let temp: Lit = handle_lit(lit, vars_added, inst_in_use);
        clause.add(temp);
    }

    inst_in_use.add_clause(clause);
}

pub fn handle_cnf(
    vec_cnf: &Vec<Expression>,
    vars_added: &mut HashMap<String, Lit>,
    finds: Vec<String>,
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

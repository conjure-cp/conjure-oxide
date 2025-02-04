use anyhow::{anyhow, Result};
use rustsat::instances::SatInstance;
use rustsat::types::{Clause, Lit, Var};
use std::collections::HashMap;

use crate::ast::{self, Atom, Expression, Name};
use crate::metadata::Metadata;
// use crate::error::Error;
use crate::solver::Error;
use crate::solver::SolverError;
use crate::Model as ConjureModel;
// use crate::;

pub fn instantiate_model_from_conjure(
    conjure_model: ConjureModel,
) -> Result<SatInstance, SolverError> {
    let mut inst: SatInstance = SatInstance::new();

    let md = Metadata {
        clean: false,
        etype: None,
    };

    let constraints_vec: Vec<Expression> = conjure_model.get_constraints_vec();
    let vec_cnf = handle_and(Expression::And(md, constraints_vec));
    conv_to_formula(&(vec_cnf.unwrap()), &mut inst);

    Ok(inst)
}

pub fn handle_expr(e: Expression) -> Result<(Vec<Vec<i32>>), CNFError> {
    match e {
        Expression::And(_, _) => Ok(handle_and(e).unwrap()),
        _ => Err(CNFError::UnexpectedExpression(e)),
    }
}

pub fn get_atom_as_int(atom: Atom) -> Result<i32, CNFError> {
    match atom {
        Atom::Literal(literal) => todo!(),
        Atom::Reference(name) => match name {
            Name::MachineName(val) => Ok(val),
            _ => Err(CNFError::BadVariableType(name)),
        },
    }
}

pub fn handle_lit(e: Expression) -> Result<i32, CNFError> {
    match e {
        Expression::Not(_, heap_expr) => {
            let expr = *heap_expr;
            match expr {
                Expression::Not(_md, e) => handle_lit(*e),
                // todo(ss504): decide
                Expression::Atomic(_, atom) => {
                    let check = get_atom_as_int(atom).unwrap();
                    match check == 0 {
                        true => Ok(1),
                        false => Ok(0),
                    }
                }
                _ => Err(CNFError::UnexpectedExpressionInsideNot(expr)),
            }
        }
        Expression::Atomic(_md, atom) => get_atom_as_int(atom),
        _ => Err(CNFError::UnexpectedLiteralExpression(e)),
    }
}

pub fn handle_or(e: Expression) -> Result<(Vec<i32>), CNFError> {
    let vec_clause = match e {
        Expression::Or(_md, vec) => vec,
        _ => Err(CNFError::UnexpectedExpression(e))?,
    };

    let mut ret_clause: Vec<i32> = Vec::new();

    for expr in vec_clause {
        match expr {
            Expression::Atomic(_, _) => ret_clause.push(handle_lit(expr).unwrap()),
            Expression::Not(_, _) => ret_clause.push(handle_lit(expr).unwrap()),
            _ => Err(CNFError::UnexpectedExpressionInsideOr(expr))?,
        }
    }

    Ok(ret_clause)
}

pub fn handle_and(e: Expression) -> Result<(Vec<Vec<i32>>), CNFError> {
    let vec_cnf = match e {
        Expression::And(_md, vec_and) => vec_and,
        _ => panic!("Villain, What hast thou done?\nThat which thou canst not undo."),
    };

    let mut ret_vec_of_vecs: Vec<Vec<i32>> = Vec::new();

    for expr in vec_cnf {
        match expr {
            Expression::Or(_, _) => ret_vec_of_vecs.push(handle_or(expr).unwrap()),
            _ => Err(CNFError::UnexpectedExpressionInsideOr(expr))?,
        }
    }

    Ok(ret_vec_of_vecs)
}

pub fn conv_to_clause(
    to_convert: &Vec<i32>,
    instance_in_use: &mut SatInstance,
    var_map: &mut HashMap<i32, Var>,
) -> Result<()> {
    let lits: Vec<Lit> = to_convert
        .iter()
        .map(|&num| mk_lit(num, instance_in_use, var_map))
        .collect::<Result<Vec<Lit>, anyhow::Error>>()?;
    let clause: Clause = lits.into_iter().collect();
    instance_in_use.add_clause(clause);
    Ok(())
}

pub fn mk_lit(
    num: i32,
    instance_in_use: &mut SatInstance,
    var_map: &mut HashMap<i32, Var>,
) -> Result<Lit, anyhow::Error> {
    if num == 0 {
        return Err(anyhow!("Variable index cannot be zero. Received: {}", num));
    }

    let var_index = num.abs();
    let var = if let Some(&v) = var_map.get(&var_index) {
        v
    } else {
        let v = instance_in_use.new_var();
        var_map.insert(var_index, v);
        v
    };
    if num > 0 {
        Ok(var.pos_lit())
    } else {
        Ok(var.neg_lit())
    }
}

pub fn conv_to_formula(vec_cnf: &Vec<Vec<i32>>, instance_in_use: &mut SatInstance) -> Result<()> {
    let mut var_map: HashMap<i32, Var> = HashMap::new();
    for clause in vec_cnf {
        conv_to_clause(clause, instance_in_use, &mut var_map)?;
    }
    Ok(())
}

//CNF Error, may be replaced of integrated with error file
#[derive(Error, Debug)]
pub enum CNFError {
    #[error("Variable with name `{0}` not found")]
    VariableNameNotFound(Name),

    #[error("Variable with name `{0}` not of right type")]
    BadVariableType(Name),

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

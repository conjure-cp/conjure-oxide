use std::any::type_name;
use std::fmt::format;
use std::iter::Inspect;
use std::ptr::null;
use std::vec;

use minion_rs::ast::Model;
use rustsat::encodings::am1::Def;
use rustsat::solvers::{Solve, SolverResult};
use rustsat::types::Var as satVar;
use sat_rs::sat_tree::{self, conv_to_clause, conv_to_formula};
use std::collections::HashMap;

use rustsat_minisat::core::Minisat;

use crate::ast::{Expression, Name};
use crate::metadata::Metadata;
use crate::solver::{self, SolveSuccess, SolverCallback, SolverFamily, SolverMutCallback};
use crate::{ast as conjure_ast, model, Model as ConjureModel};

use super::super::model_modifier::NotModifiable;
use super::super::private;
use super::super::SearchComplete::*;
use super::super::SearchIncomplete::*;
use super::super::SearchStatus::*;
use super::super::SolverAdaptor;
use super::super::SolverError;
use super::super::SolverError::*;
use super::super::SolverError::*;

use rustsat::instances::SatInstance;

// use anyhow::Error;
use thiserror::Error;

/// A [SolverAdaptor] for interacting with the SatSolver generic and the types thereof.

pub struct SAT {
    __non_constructable: private::Internal,
    model_inst: Option<SatInstance>,
    var_map: Option<HashMap<i32, satVar>>,
    solver: Option<Minisat>,
}

impl private::Sealed for SAT {}

impl Default for SAT {
    fn default() -> Self {
        SAT {
            __non_constructable: private::Internal,
            model_inst: None,
            var_map: None,
            solver: Some(Minisat::default()),
        }
    }
}

impl SAT {
    pub fn new(model: ConjureModel) -> Self {
        let model_to_use: Option<SatInstance> = Some(SatInstance::new());
        SAT {
            __non_constructable: private::Internal,
            model_inst: model_to_use,
            var_map: None,
            solver: Some(Minisat::default()),
        }
    }

    pub fn add_clause_to_mod(&self, clause_vec: Vec<i32>) -> () {}
}

pub fn instantiate_model_from_conjure(
    conjure_model: ConjureModel,
) -> Result<SatInstance, SolverError> {
    let mut inst: SatInstance = SatInstance::new();

    for var_name_ref in conjure_model.variables.keys() {
        let curr_decision_var = conjure_model
            .variables
            .get(var_name_ref)
            .ok_or_else(|| ModelInvalid(format!("variable {:?} not found", var_name_ref)))?;

        // process decision var

        {
            // todo: the scope change may be unneeded
            // check domain, err if bad domain
            let cdom = &curr_decision_var.domain;
            if cdom != &conjure_ast::Domain::BoolDomain {
                return Err(ModelFeatureNotSupported(format!(
                    "variable {:?}: expected BoolDomain, found: {:?}",
                    curr_decision_var, curr_decision_var.domain
                )));
            }
        }
    }

    let md = Metadata {
        clean: false,
        etype: None,
    };

    let constraints_vec: Vec<Expression> = conjure_model.get_constraints_vec();
    let vec_cnf = handle_and(Expression::And(md, constraints_vec));
    conv_to_formula(&(vec_cnf.unwrap()), &mut inst);

    Ok(inst)
}

impl SolverAdaptor for SAT {
    fn solve(
        &mut self,
        callback: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        // solver = self.solver
        let cnf_func = self.model_inst.clone().unwrap().into_cnf();
        // let res = self.solver.clone().unwrap().add_cnf(cnf_func.0);

        /**
         * todo (ss504 + solver backend): ask at meeting
         * Immediate:
         *      Fix formatting for SolverSuccess
         *      Fix Solver call
         * Future:
         *      Fix Solver call to implement generic solver type or traits to universalize the solver
         *      implement new solvers
         */
        // Ok(SolveSuccess::)
        Err(OpNotImplemented("solve_mut".to_owned()))
    }

    fn solve_mut(
        &mut self,
        callback: SolverMutCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(OpNotSupported("solve_mut".to_owned()))
    }

    fn load_model(&mut self, model: ConjureModel, _: private::Internal) -> Result<(), SolverError> {
        // todo (ss504 + backend) fix error handling
        let inst_res = instantiate_model_from_conjure(model);
        self.model_inst = Some(inst_res.unwrap());
        Ok(())
    }

    fn get_family(&self) -> SolverFamily {
        SolverFamily::SAT
    }
}

pub fn handle_expr(e: Expression) -> Result<(Vec<Vec<i32>>), CNFError> {
    // todo(ss504): Add support for clause-only, literal only and empty expressions
    match e {
        Expression::And(_, _) => Ok(handle_and(e).unwrap()),
        // Expression::Or(_,_) => handle_or(e),
        // Expression::Not(_, _) => handle_lit(e),
        // Expression::Reference(, ) => handle_lit(e)
        // Expression::Nothing() => None,
        _ => Err(CNFError::UnexpectedExpression(e)), // panic!("Villain, What hast thou done?\nThat which thou canst not undo.")
    }
}

pub fn get_namevar_as_int(name: Name) -> Result<i32, CNFError> {
    match name {
        Name::MachineName(val) => Ok(val),
        _ => Err(CNFError::BadVariableType(name)),
        // panic!("Villain, What hast thou done?\nThat which thou canst not undo."),
    }
}

pub fn handle_lit(e: Expression) -> Result<i32, CNFError> {
    match e {
        Expression::Not(_, heap_expr) => {
            let expr = *heap_expr;
            match expr {
                Expression::Nothing => todo!(), // panic?
                Expression::Not(_md, e) => handle_lit(*e),
                // todo(ss504): decide
                // Expression::Reference(_md, name) => get_namevar_as_int(name) * -1,
                Expression::Reference(_md, name) => {
                    let check = get_namevar_as_int(name).unwrap();
                    match check == 0 {
                        true => Ok(1),
                        false => Ok(0),
                    }
                }
                _ => Err(CNFError::UnexpectedExpressionInsideNot(expr)),
            }
        }
        Expression::Reference(_md, name) => get_namevar_as_int(name),
        _ => Err(CNFError::UnexpectedLiteralExpression(e)),
        // _ => panic!("Villain, What hast thou done?\nThat which thou canst not undo."),
    }
}

pub fn handle_or(e: Expression) -> Result<(Vec<i32>), CNFError> {
    let vec_clause = match e {
        Expression::Or(_md, vec) => vec,
        // _ => panic!("Villain, What hast thou done?\nThat which thou canst not undo."),
        _ => Err(CNFError::UnexpectedExpression(e))?,
    };

    if vec_clause.len() != 2 {
        panic!("Villain, What hast thou done?\nThat which thou canst not undo.")
    };

    let mut ret_clause: Vec<i32> = Vec::new();

    for expr in vec_clause {
        match expr {
            Expression::Reference(_, _) => ret_clause.push(handle_lit(expr).unwrap()),
            Expression::Not(_, _) => ret_clause.push(handle_lit(expr).unwrap()),
            _ => Err(CNFError::UnexpectedExpressionInsideOr(expr))?,
        }
    }

    Ok(ret_clause)
}

pub fn handle_and(e: Expression) -> Result<(Vec<Vec<i32>>), CNFError> {
    let vec_cnf = match e {
        // todo (ss504) check for nested ands
        Expression::And(_md, vec_and) => vec_and,
        Expression::Not(_md, e) => todo!(),
        Expression::Or(_md, vec_or) => todo!(),
        Expression::Reference(_md, e) => todo!(),

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
//CNF Error, may be replaced of integrated with error file
#[derive(Error, Debug)]
pub enum CNFError {
    // #[error("Variable with name `{0}` not found")]
    // VariableNameNotFound(conjure_ast::Name),
    #[error("Variable with name `{0}` not of right type")]
    BadVariableType(conjure_ast::Name),

    // #[error("Clause with index `{0}` not found")]
    // ClauseIndexNotFound(i32),
    #[error("Unexpected Expression `{0}` inside Not(). Only Not(Reference) or Not(Not) allowed!")]
    UnexpectedExpressionInsideNot(conjure_ast::Expression),

    #[error("Unexpected Expression `{0}` as literal. Only Not() or Reference() allowed!")]
    UnexpectedLiteralExpression(conjure_ast::Expression),

    #[error("Unexpected Expression `{0}` inside And(). Only And(vec<Or>) allowed!")]
    UnexpectedExpressionInsideAnd(conjure_ast::Expression),

    #[error("Unexpected Expression `{0}` inside Or(). Only Or( ) allowed!")]
    UnexpectedExpressionInsideOr(conjure_ast::Expression),

    // todo: change msg to suit
    #[error("Unexpected Expression `{0}` found!")]
    UnexpectedExpression(conjure_ast::Expression),

    #[error("Unexpected nested And: {0}")]
    NestedAnd(conjure_ast::Expression),
}

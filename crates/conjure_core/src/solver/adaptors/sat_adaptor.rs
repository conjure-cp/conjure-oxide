use std::any::type_name;
use std::fmt::format;
use std::iter::Inspect;
use std::ptr::null;
use std::vec;

use minion_rs::ast::Model;
use rustsat::encodings::am1::Def;
use rustsat::solvers::SolverResult;
use rustsat::types::Var as satVar;
use rustsat_minisat::core::Minisat;
use sat_rs::sat_tree::{self, conv_to_clause};
use std::collections::HashMap;

use crate::ast::Expression;
use crate::solver::{SolveSuccess, SolverCallback, SolverFamily, SolverMutCallback};
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
use super::sat_common::CNFModel;

use rustsat::instances::SatInstance;

use thiserror::Error;

/// A [SolverAdaptor] for interacting with the SatSolver generic and the types thereof.

pub struct SAT {
    __non_constructable: private::Internal,
    model_inst: Option<SatInstance>,
    var_map: Option<HashMap<i32, satVar>>,
}

impl private::Sealed for SAT {}

impl Default for SAT {
    fn default() -> Self {
        SAT {
            __non_constructable: private::Internal,
            model_inst: Some(SatInstance::new()),
            var_map: None,
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
        }
    }

    pub fn add_clause_to_mod(&self, clause_vec: Vec<i32>) -> () {}

    pub fn from_conjure(conjure_model: ConjureModel) -> Result<SatInstance, SolverError> {
        let mut inst: SatInstance = SatInstance::new();

        for var_name_ref in conjure_model.variables.keys() {
            let curr_decision_var = conjure_model
                .variables
                .get(var_name_ref)
                .ok_or_else(|| ModelInvalid(format!("variable {:?} not found", var_name_ref)))?;

            // process decision var

            {
                // todo: the scope change may be dumb as shit
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

        let mut vector_constraints_in_cnf: Vec<Vec<i32>> = Vec::new();
        for e in conjure_model.get_constraints_vec() {
            // hell
            // add expression to vec
            match e {
                Expression::And(_, _) =>
                // add and
                {
                    Err(OpNotImplemented(format!(
                        "{:?} operations not implemented",
                        e
                    )))?
                }
                Expression::Not(_, _) =>
                // add_not
                {
                    Err(OpNotImplemented(format!(
                        "{:?} operations not implemented",
                        e
                    )))?
                }
                Expression::Or((_), (_)) =>
                // add_or()
                {
                    Err(OpNotImplemented(format!(
                        "{:?} operations not implemented",
                        e
                    )))?
                }
                // default case, incompatible type
                _ => {
                    Err(OpNotSupported((
                        // no oxford commmas in this Uni
                        format!(
                            "Bad Constraint: found type.\n
                            SAT does not support constraints of type other than Expession::Not, Expression::And or Expression::Or."
                        )
                    )))?
                }
            }
        }

        Err(OpNotImplemented((format!(""))))
    }
}

impl SolverAdaptor for SAT {
    fn solve(
        &mut self,
        callback: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        // ToDo (ss504): this needs to be fixed after load_model
        Err(OpNotSupported("solve_mut".to_owned()))
        // ToDo (sat_backend): check res for satisfiability and init Result<SolveSuccess, SolverError>
    }

    fn solve_mut(
        &mut self,
        callback: SolverMutCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(OpNotSupported("solve_mut".to_owned()))
    }

    fn load_model(&mut self, model: ConjureModel, _: private::Internal) -> Result<(), SolverError> {
        // ToDo (ss504) use the sat_tree functions to create an instance (do not need to use model). Return Result<SatInstance, SolveError>
        Err(OpNotSupported("solve_mut".to_owned()))
    }

    fn get_family(&self) -> SolverFamily {
        SolverFamily::SAT
    }
}

// TODO (ss504): FIX ERROR TYPES
fn handle_not(e: Expression) -> Result<bool, SolverError> {
    let val: Box<Expression> = match e {
        Expression::Not(_, ref boxed) => *boxed,
        _ => OpNotSupported(format!(
            "Wrong Function for {:?}, use function to handle variant.",
            e
        )),
    };

    match val {
        Expression::Not => handle_not(val),
        Expression::Constant => {
            let ret: bool = match {
                
            }
            ret?
        }
        _ => Err(OpNotSupported(
            (format!("Wrong variant in expression for e")),
        ))?,
    }
}

//CNF Error, may be replaced of integrated with error file
#[derive(Error, Debug)]
pub enum CNFError {
    #[error("Variable with name `{0}` not found")]
    VariableNameNotFound(conjure_ast::Name),

    #[error("Clause with index `{0}` not found")]
    ClauseIndexNotFound(i32),

    #[error("Unexpected Expression `{0}` inside Not(). Only Not(Reference) allowed!")]
    UnexpectedExpressionInsideNot(conjure_ast::Expression),

    #[error(
        "Unexpected Expression `{0}` found. Only Reference, Not(Reference) and Or(...) allowed!"
    )]
    UnexpectedExpression(conjure_ast::Expression),

    #[error("Unexpected nested And: {0}")]
    NestedAnd(conjure_ast::Expression),
}

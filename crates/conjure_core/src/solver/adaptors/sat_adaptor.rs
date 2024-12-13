use std::any::type_name;
use std::fmt::format;
use std::iter::Inspect;
use std::ptr::null;
use std::vec;

use clap::error;
use minion_rs::ast::Model;
use rustsat::encodings::am1::Def;
use rustsat::solvers::{Solve, SolverResult};
use rustsat::types::{Var as satVar, Clause, Lit}; 
use sat_rs::sat_tree::{self, conv_to_clause, conv_to_formula};
use std::collections::{HashMap, VecDeque}; 
use crate::context::Context;

use rustsat_minisat::core::Minisat;

use crate::ast::{Expression, Name};
use crate::metadata::Metadata;
use crate::solver::{self, SearchStatus, SolveSuccess, SolverCallback, SolverFamily, SolverMutCallback};
use crate::stats::SolverStats;
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
use thiserror::Error;

/// A [SolverAdaptor] for interacting with the SatSolver generic and the types thereof.

pub struct SAT {
    __non_constructable: private::Internal,
    model_inst: Option<SatInstance>,
    var_map: Option<HashMap<i32, satVar>>,         // Mapping ConjureModel -> Minisat variables
    reverse_var_map: Option<HashMap<satVar, i32>>, // Reverse mapping Minisat -> ConjureModel variables
    solver_inst: Minisat,
    incremental_clauses: VecDeque<Clause>,
}

impl private::Sealed for SAT {}

impl Default for SAT {
    fn default() -> Self {
        SAT {
            __non_constructable: private::Internal,
            model_inst: None,
            var_map: None,
            reverse_var_map: None,
            solver_inst: Minisat::default(),
            incremental_clauses: VecDeque::new(),
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
            reverse_var_map: None,
            solver_inst: Minisat::default(),
            incremental_clauses: VecDeque::new(),
        }
    }

    // Adds a new clause to the SAT solver incrementally.
    pub fn add_incremental_clause(&mut self, clause: Vec<i32>) -> Result<(), SolverError> {
        let new_clause: Clause = clause
            .iter()
            .map(|&lit| {
                if lit == 0 {
                    panic!("Literal value 0 is not allowed.");
                }
                let abs_lit = lit.abs() as u32;
                if lit > 0 {
                    Lit::positive(abs_lit)
                } else {
                    Lit::negative(abs_lit)
                }
            })
            .collect();

        // Add the clause to the deque for keeping records
        self.incremental_clauses.push_back(new_clause.clone());
        println!("Added incremental clause: {:?}", clause);
        // Added the clause directly to Minisat
        match self.solver_inst.add_clause(new_clause) {
            Ok(_) => Ok(()),
            Err(e) => Err(SolverError::Runtime(format!("{:?}", e))),
        }
    }

    pub fn get_sat_var(&self, model_var: i32) -> Option<satVar> {
        self.var_map.as_ref()?.get(&model_var).copied()
    }

    pub fn get_model_var(&self, sat_var: satVar) -> Option<i32> {
        self.reverse_var_map.as_ref()?.get(&sat_var).copied()
    }

    pub fn add_clause_to_mod(&self, clause_vec: Vec<i32>) -> () {}
}

pub fn instantiate_model_from_conjure(
    conjure_model: ConjureModel,
) -> Result<(SatInstance, HashMap<i32, satVar>, HashMap<satVar, i32>), SolverError> {
    let mut inst: SatInstance = SatInstance::new();
    let mut var_map: HashMap<i32, satVar> = HashMap::new();
    let mut reverse_var_map: HashMap<satVar, i32> = HashMap::new();

    for (var_name_ref, decision_var) in conjure_model.variables.iter() {
        let cdom = &decision_var.domain;
        if cdom != &conjure_ast::Domain::BoolDomain {
            return Err(ModelFeatureNotSupported(format!(
                "variable {:?}: expected BoolDomain but found: {:?}",
                decision_var, decision_var.domain
            )));
        }

        let sat_var = inst.new_var();
        let var_id = match var_name_ref {
            Name::MachineName(id) => *id, // Extracts integer ID from Name
            _ => {
                println!("Unsupported variable name format: {:?}", var_name_ref);
                return Err(SolverError::Runtime(format!(
                    "Unsupported variable name format: {:?}",
                    var_name_ref
                )));
            }
        };

        var_map.insert(var_id, sat_var);
        reverse_var_map.insert(sat_var, var_id);
    }
    let constraints_vec: Vec<Expression> = conjure_model.get_constraints_vec();
    let vec_cnf = handle_and(Expression::And(
        Metadata {
            clean: false,
            etype: None,
        },
        constraints_vec,
    ));
    conv_to_formula(&(vec_cnf.unwrap()), &mut inst);

    Ok((inst, var_map, reverse_var_map))
}

impl SolverAdaptor for SAT {

    fn load_model(&mut self, model: ConjureModel, _: private::Internal) -> Result<(), SolverError> {
        let (inst, var_map, reverse_var_map) = instantiate_model_from_conjure(model)?;
        self.model_inst = Some(inst);
        self.var_map = Some(var_map);
        self.reverse_var_map = Some(reverse_var_map);
        Ok(())
    }

    fn solve(
        &mut self,
        callback: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        // Extracts CNF from the model instance
        let cnf_func = self.model_inst.clone()
            .ok_or_else(|| SolverError::Runtime("No model instance found".to_owned()))?
            .into_cnf()
            .0;
    
        // Then adds CNF to the solver instance
        self.solver_inst.add_cnf(cnf_func).map_err(|e| {
            SolverError::Runtime(format!("Failed to add CNF to the solver: {:?}", e))
        })?;
    
        // Now solving the SAT problem
        let res = self.solver_inst.solve().map_err(|e| {
            SolverError::Runtime(format!("Solver faced an error: {:?}", e))
        })?;
    
        let solver_res = match res {
            SolverResult::Sat => true,
            SolverResult::Unsat => false,
            SolverResult::Interrupted => {
                return Err(SolverError::Runtime(
                    "SatInstance may be invalid: Interrupted.".to_owned(),
                ));
            }
        };
    
        // Gives SAT results
        if solver_res {
            if let Ok(solution) = self.solver_inst.full_solution() {
                println!("SAT Solution: {:?}", solution);
            } else {
                println!("Unable to get the full solution from the solver.");
            }
        }
        if !solver_res {
            return Err(SolverError::Runtime(
                "UNSAT result faced as expected.".to_owned(),
            ));
        }
    
        println!("{}", solver_res);
        Err(OpNotImplemented("solve_mut".to_owned()))
    }    
    
    fn solve_mut(
        &mut self,
        callback: SolverMutCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(OpNotSupported("solve_mut".to_owned()))
    }

    fn get_family(&self) -> SolverFamily {
        SolverFamily::SAT
    }
}

pub fn handle_expr(e: Expression) -> Result<(Vec<Vec<i32>>), CNFError> {
    match e {
        Expression::And(_, _) => Ok(handle_and(e).unwrap()),
        _ => Err(CNFError::UnexpectedExpression(e)),
    }
}

pub fn get_namevar_as_int(name: Name) -> Result<i32, CNFError> {
    match name {
        Name::MachineName(val) => Ok(val),
        _ => Err(CNFError::BadVariableType(name)),
    }
}

pub fn handle_lit(e: Expression) -> Result<i32, CNFError> {
    match e {
        Expression::Not(_, heap_expr) => {
            let expr = *heap_expr;
            match expr {
                Expression::Reference(_md, name) => {
                    let lit_value = get_namevar_as_int(name)?;
                    Ok(-lit_value)
                }
                Expression::Not(_, inner_expr) => handle_lit(*inner_expr),
                _ => Err(CNFError::UnexpectedExpressionInsideNot(expr)),
            }
        }
        Expression::Reference(_md, name) => get_namevar_as_int(name),
        _ => Err(CNFError::UnexpectedLiteralExpression(e)),
    }
}

pub fn handle_or(e: Expression) -> Result<Vec<i32>, CNFError> {
    let vec_clause = match &e {
        Expression::Or(_, vec) => vec,
        _ => return Err(CNFError::UnexpectedExpression(e.clone())),
    };

    let mut ret_clause: Vec<i32> = Vec::new();

    for expr in vec_clause {
        match expr {
            Expression::Reference(_, _) => ret_clause.push(handle_lit(expr.clone())?),
            Expression::Not(_, _) => ret_clause.push(handle_lit(expr.clone())?),
            Expression::Or(_, _) => return Err(CNFError::UnexpectedExpressionInsideOr(expr.clone())),
            _ => return Err(CNFError::UnexpectedExpressionInsideOr(expr.clone())),
        }
    }

    if ret_clause.is_empty() {
        return Err(CNFError::UnexpectedExpressionInsideOr(e));
    }

    Ok(ret_clause)
}

pub fn handle_and(e: Expression) -> Result<Vec<Vec<i32>>, CNFError> {
    let vec_cnf = match &e {
        Expression::And(_, vec_and) => vec_and,
        _ => return Err(CNFError::UnexpectedExpression(e.clone())),
    };

    let mut ret_vec_of_vecs: Vec<Vec<i32>> = Vec::new();

    for expr in vec_cnf {
        match expr {
            Expression::Or(_, _) => ret_vec_of_vecs.push(handle_or(expr.clone())?),
            Expression::Reference(_, _) | Expression::Not(_, _) => {
                ret_vec_of_vecs.push(vec![handle_lit(expr.clone())?])
            }
            _ => return Err(CNFError::UnexpectedExpressionInsideAnd(expr.clone())),
        }
    }
    Ok(ret_vec_of_vecs)
}

fn dynamic_clause_addition(sat_solver: &mut SAT) {
    let dynamic_clause = vec![1, -2, 3];
    if let Err(e) = sat_solver.add_incremental_clause(dynamic_clause) {
        println!("Error adding clause: {:?}", e);
    }
}

//CNF Error, may be replaced of integrated with error file
#[derive(Error, Debug)]
pub enum CNFError {
    #[error("Variable with name {0} not found")]
    VariableNameNotFound(conjure_ast::Name),

    #[error("Variable with name {0} not of right type")]
    BadVariableType(Name),

    #[error("Unexpected Expression {0} inside Not(). Only Not(Reference) or Not(Not) allowed!")]
    UnexpectedExpressionInsideNot(Expression),

    #[error("Unexpected Expression {0} as literal. Only Not() or Reference() allowed!")]
    UnexpectedLiteralExpression(Expression),

    #[error("Unexpected Expression {0} inside And(). Only And(vec<Or>) allowed!")]
    UnexpectedExpressionInsideAnd(Expression),

    #[error("Unexpected Expression {0} inside Or(). Only Or(lit, lit) allowed!")]
    UnexpectedExpressionInsideOr(Expression),

    #[error("Unexpected Expression {0} found!")]
    UnexpectedExpression(Expression)
}
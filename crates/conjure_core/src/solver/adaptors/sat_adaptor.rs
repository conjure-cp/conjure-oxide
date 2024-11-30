// use std::any::type_name;
// use std::fmt::format;
// use std::iter::Inspect;
// use std::ptr::null;
// use std::vec;

// use clap::error;
// use minion_rs::ast::Model;
// use rustsat::encodings::am1::Def;
// use rustsat::solvers::{Solve, SolverResult};
// use rustsat::types::Var as satVar;
// use sat_rs::sat_tree::{self, conv_to_clause, conv_to_formula};
// use std::collections::HashMap;

// use rustsat_minisat::core::Minisat;

// use crate::ast::{Expression, Name};
// use crate::metadata::Metadata;
// use crate::solver::{self, SolveSuccess, SolverCallback, SolverFamily, SolverMutCallback};
// use crate::{ast as conjure_ast, model, Model as ConjureModel};

// use super::super::model_modifier::NotModifiable;
// use super::super::private;
// use super::super::SearchComplete::*;
// use super::super::SearchIncomplete::*;
// use super::super::SearchStatus::*;
// use super::super::SolverAdaptor;
// use super::super::SolverError;
// use super::super::SolverError::*;
// use super::super::SolverError::*;

// use rustsat::instances::SatInstance;

// // use anyhow::Error;
// use thiserror::Error;

// /// A [SolverAdaptor] for interacting with the SatSolver generic and the types thereof.

// pub struct SAT {
//     __non_constructable: private::Internal,
//     model_inst: Option<SatInstance>,
//     var_map: Option<HashMap<i32, satVar>>,
//     solver_inst: Option<Minisat>,
// }

// impl private::Sealed for SAT {}

// impl Default for SAT {
//     fn default() -> Self {
//         SAT {
//             __non_constructable: private::Internal,
//             model_inst: None,
//             var_map: None,
//             solver_inst: Some(Minisat::default()),
//         }
//     }
// }

// impl SAT {
//     pub fn new(model: ConjureModel) -> Self {
//         let model_to_use: Option<SatInstance> = Some(SatInstance::new());
//         SAT {
//             __non_constructable: private::Internal,
//             model_inst: model_to_use,
//             var_map: None,
//             solver_inst: Some(Minisat::default()),
//         }
//     }

//     pub fn add_clause_to_mod(&self, clause_vec: Vec<i32>) -> () {}
// }

// pub fn instantiate_model_from_conjure(
//     conjure_model: ConjureModel,
// ) -> Result<SatInstance, SolverError> {
//     let mut inst: SatInstance = SatInstance::new();

//     for var_name_ref in conjure_model.variables.keys() {
//         let curr_decision_var = conjure_model
//             .variables
//             .get(var_name_ref)
//             .ok_or_else(|| ModelInvalid(format!("variable {:?} not found", var_name_ref)))?;

//         // process decision var

//         {
//             // todo: the scope change may be unneeded
//             // check domain, err if bad domain
//             let cdom = &curr_decision_var.domain;
//             if cdom != &conjure_ast::Domain::BoolDomain {
//                 return Err(ModelFeatureNotSupported(format!(
//                     "variable {:?}: expected BoolDomain, found: {:?}",
//                     curr_decision_var, curr_decision_var.domain
//                 )));
//             }
//         }
//     }

//     let md = Metadata {
//         clean: false,
//         etype: None,
//     };

//     let constraints_vec: Vec<Expression> = conjure_model.get_constraints_vec();
//     let vec_cnf = handle_and(Expression::And(md, constraints_vec));
//     conv_to_formula(&(vec_cnf.unwrap()), &mut inst);

//     Ok(inst)
// }

// impl SolverAdaptor for SAT {
//     fn solve(
//         &mut self,
//         callback: SolverCallback,
//         _: private::Internal,
//     ) -> Result<SolveSuccess, SolverError> {
//         // solver = self.solver
//         // handle
//         let cnf_func = self.model_inst.clone().unwrap().into_cnf();
//         // let res = self.solver.clone().unwrap().add_cnf(cnf_func.0);

//         /**
//          * todo (ss504 + solver backend): ask at meeting
//          * Immediate:
//          *      Fix formatting for SolverSuccess
//          *      Fix Solver call
//          * Future:
//          *      Fix Solver call to implement generic solver type or traits to universalize the solver
//          *      implement new solvers
//          */
//         // Ok(SolveSuccess::)
//         Err(OpNotImplemented("solve_mut".to_owned()))
//     }

//     fn solve_mut(
//         &mut self,
//         callback: SolverMutCallback,
//         _: private::Internal,
//     ) -> Result<SolveSuccess, SolverError> {
//         Err(OpNotSupported("solve_mut".to_owned()))
//     }

//     fn load_model(&mut self, model: ConjureModel, _: private::Internal) -> Result<(), SolverError> {
//         let inst_res = instantiate_model_from_conjure(model);
//         self.model_inst = Some(inst_res.unwrap());
//         Ok(())
//     }

//     fn get_family(&self) -> SolverFamily {
//         SolverFamily::SAT
//     }
// }

// pub fn handle_expr(e: Expression) -> Result<(Vec<Vec<i32>>), CNFError> {
//     match e {
//         Expression::And(_, _) => Ok(handle_and(e).unwrap()),
//         _ => Err(CNFError::UnexpectedExpression(e)),
//     }
// }

// pub fn get_namevar_as_int(name: Name) -> Result<i32, CNFError> {
//     match name {
//         Name::MachineName(val) => Ok(val),
//         _ => Err(CNFError::BadVariableType(name)),
//         // panic!("Villain, What hast thou done?\nThat which thou canst not undo."),
//     }
// }

// pub fn handle_lit(e: Expression) -> Result<i32, CNFError> {
//     match e {
//         Expression::Not(_, heap_expr) => {
//             let expr = *heap_expr;
//             match expr {
//                 Expression::Nothing => todo!(), // panic?
//                 Expression::Not(_md, e) => handle_lit(*e),
//                 // todo(ss504): decide
//                 // Expression::Reference(_md, name) => get_namevar_as_int(name) * -1,
//                 Expression::Reference(_md, name) => {
//                     let check = get_namevar_as_int(name).unwrap();
//                     match check == 0 {
//                         true => Ok(1),
//                         false => Ok(0),
//                     }
//                 }
//                 _ => Err(CNFError::UnexpectedExpressionInsideNot(expr)),
//             }
//         }
//         Expression::Reference(_md, name) => get_namevar_as_int(name),
//         _ => Err(CNFError::UnexpectedLiteralExpression(e)),
//         // _ => panic!("Villain, What hast thou done?\nThat which thou canst not undo."),
//     }
// }

// pub fn handle_or(e: Expression) -> Result<(Vec<i32>), CNFError> {
//     let vec_clause = match e {
//         Expression::Or(_md, vec) => vec,
//         // _ => panic!("Villain, What hast thou done?\nThat which thou canst not undo."),
//         _ => Err(CNFError::UnexpectedExpression(e))?,
//     };

//     if vec_clause.len() != 2 {
//         panic!("Villain, What hast thou done?\nThat which thou canst not undo.")
//     };

//     let mut ret_clause: Vec<i32> = Vec::new();

//     for expr in vec_clause {
//         match expr {
//             Expression::Reference(_, _) => ret_clause.push(handle_lit(expr).unwrap()),
//             Expression::Not(_, _) => ret_clause.push(handle_lit(expr).unwrap()),
//             _ => Err(CNFError::UnexpectedExpressionInsideOr(expr))?,
//         }
//     }

//     Ok(ret_clause)
// }

// pub fn handle_and(e: Expression) -> Result<(Vec<Vec<i32>>), CNFError> {
//     let vec_cnf = match e {
//         Expression::And(_md, vec_and) => vec_and,

//         _ => panic!("Villain, What hast thou done?\nThat which thou canst not undo."),
//     };

//     let mut ret_vec_of_vecs: Vec<Vec<i32>> = Vec::new();

//     for expr in vec_cnf {
//         match expr {
//             Expression::Or(_, _) => ret_vec_of_vecs.push(handle_or(expr).unwrap()),
//             _ => Err(CNFError::UnexpectedExpressionInsideOr(expr))?,
//         }
//     }

//     Ok(ret_vec_of_vecs)
// }
// //CNF Error, may be replaced of integrated with error file
// #[derive(Error, Debug)]
// pub enum CNFError {
//     #[error("Variable with name `{0}` not found")]
//     VariableNameNotFound(conjure_ast::Name),

//     #[error("Variable with name `{0}` not of right type")]
//     BadVariableType(Name),

//     // #[error("Clause with index `{0}` not found")]
//     // ClauseIndexNotFound(i32),
//     #[error("Unexpected Expression `{0}` inside Not(). Only Not(Reference) or Not(Not) allowed!")]
//     UnexpectedExpressionInsideNot(Expression),

//     #[error("Unexpected Expression `{0}` as literal. Only Not() or Reference() allowed!")]
//     UnexpectedLiteralExpression(Expression),

//     #[error("Unexpected Expression `{0}` inside And(). Only And(vec<Or>) allowed!")]
//     UnexpectedExpressionInsideAnd(Expression),

//     #[error("Unexpected Expression `{0}` inside Or(). Only Or( ) allowed!")]
//     UnexpectedExpressionInsideOr(Expression),

//     #[error("Unexpected Expression `{0}` found!")]
//     UnexpectedExpression(Expression)
// }

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::sync::atomic::{AtomicUsize, Ordering};

use rustsat::encodings::am1::Def;
use rustsat::solvers::{Solve, SolverResult};
use rustsat::types::{Assignment, Lit, Var as SatVar};
use rustsat_minisat::core::Minisat;

use crate::ast::{Expression, Name};
use crate::metadata::Metadata;
use crate::solver::{SolveSuccess, SolverAdaptor, SolverCallback, SolverError, SolverFamily};
use crate::{ast as conjure_ast, Model as ConjureModel};

use thiserror::Error;

pub struct SAT {
    solver: Arc<Mutex<Minisat>>,
    var_map: Arc<HashMap<i32, SatVar>>,
    activity: Arc<Mutex<HashMap<i32, f64>>>,
    decay: f64,
    conflicts: Arc<AtomicUsize>,
    decisions: Arc<AtomicUsize>,
    restarts: Arc<AtomicUsize>,
    learned_clauses: Arc<AtomicUsize>,
}

impl Default for SAT {
    fn default() -> Self {
        SAT {
            solver: Arc::new(Mutex::new(Minisat::default())),
            var_map: Arc::new(HashMap::new()),
            activity: Arc::new(Mutex::new(HashMap::new())),
            decay: 0.95,
            conflicts: Arc::new(AtomicUsize::new(0)),
            decisions: Arc::new(AtomicUsize::new(0)),
            restarts: Arc::new(AtomicUsize::new(0)),
            learned_clauses: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl SAT {
    pub fn new(model: ConjureModel) -> Result<Self, SolverError> {
        let mut sat = SAT::default();
        sat.load_model(model)?;
        Ok(sat)
    }

    fn map_literal(&self, lit: i32) -> Lit {
        let var_id = lit.abs();
        let sat_var = self.var_map.get(&var_id).expect("Variable not found in var_map");
        if lit > 0 {
            sat_var.pos_lit()
        } else {
            sat_var.neg_lit()
        }
    }

    fn bump_activity(&self, var_id: i32) {
        let mut activity = self.activity.lock().unwrap();
        let entry = activity.entry(var_id).or_insert(0.0);
        *entry += 1.0;
        if *entry > 1e100 {
            for score in activity.values_mut() {
                *score *= 1e-100;
            }
        }
    }

    fn decay_activity(&self) {
        let mut activity = self.activity.lock().unwrap();
        for score in activity.values_mut() {
            *score *= self.decay;
        }
    }

    fn select_variable(&self) -> Option<i32> {
        let activity = self.activity.lock().unwrap();
        activity
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(&var, _)| var)
    }

    fn decide(&self) -> Option<i32> {
        self.select_variable().or_else(|| {
            for (&var_id, &sat_var) in self.var_map.iter() {
                let solver = self.solver.lock().unwrap();
                if !solver.is_var_assigned(sat_var) {
                    return Some(var_id);
                }
            }
            None
        })
    }

    fn log_conflict(&self) {
        self.conflicts.fetch_add(1, Ordering::SeqCst);
        if self.conflicts.load(Ordering::SeqCst) % 100 == 0 {
            self.decay_activity();
        }
    }

    fn log_decision(&self) {
        self.decisions.fetch_add(1, Ordering::SeqCst);
    }

    fn log_learned_clause(&self) {
        self.learned_clauses.fetch_add(1, Ordering::SeqCst);
    }

    fn log_restart(&self) {
        self.restarts.fetch_add(1, Ordering::SeqCst);
    }

    fn enhance_heuristics(&self) {
        self.decay_activity();
        if let Some(var_id) = self.select_variable() {
        }
    }

    fn log_statistics(&self) {
        println!("--- Solver Statistics ---");
        println!("Conflicts: {}", self.conflicts.load(Ordering::SeqCst));
        println!("Decisions: {}", self.decisions.load(Ordering::SeqCst));
        println!("Restarts: {}", self.restarts.load(Ordering::SeqCst));
        println!("Learned Clauses: {}", self.learned_clauses.load(Ordering::SeqCst));
        println!("--------------------------");
    }
}

impl SolverAdaptor for SAT {
    fn solve(
        &mut self,
        callback: SolverCallback,
        _: crate::solver::private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        let solver_arc = Arc::clone(&self.solver);
        let var_map_arc = Arc::clone(&self.var_map);
        let activity_arc = Arc::clone(&self.activity);
        let decay = self.decay;
        let conflicts_arc = Arc::clone(&self.conflicts);
        let decisions_arc = Arc::clone(&self.decisions);
        let restarts_arc = Arc::clone(&self.restarts);
        let learned_clauses_arc = Arc::clone(&self.learned_clauses);

        let callback_arc = Arc::new(Mutex::new(callback));

        let callback_clone = Arc::clone(&callback_arc);
        let handle = thread::spawn(move || {
            let mut solver = solver_arc.lock().map_err(|_| SolverError::Interrupted)?;

            let result = solver.solve();

            match result {
                SolverResult::Sat(model) => {
                    let mut assignment = HashMap::new();
                    for (&var_id, &sat_var) in var_map_arc.iter() {
                        let value = model.value(sat_var);
                        assignment.insert(
                            var_id,
                            match value {
                                Assignment::True => true,
                                Assignment::False => false,
                                Assignment::Undef => false,
                            },
                        );
                    }

                    let mut callback = callback_clone.lock().map_err(|_| SolverError::Interrupted)?;
                    callback(assignment).map_err(|e| SolverError::CallbackError(e.to_string()))?;

                    Ok(SolveSuccess::Done)
                }
                SolverResult::Unsat => Err(SolverError::NoSolution),
                SolverResult::Interrupted => Err(SolverError::Interrupted),
            }
        });

        handle.join().map_err(|_| SolverError::Interrupted)??;

        self.log_statistics();

        Ok(SolveSuccess::Done)
    }

    fn solve_mut(
        &mut self,
        _callback: crate::solver::SolverMutCallback,
        _: crate::solver::private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(SolverError::OpNotSupported("solve_mut".to_owned()))
    }

    fn load_model(
        &mut self,
        model: ConjureModel,
        _: crate::solver::private::Internal,
    ) -> Result<(), SolverError> {
        let mut var_map = Arc::make_mut(&mut self.var_map);
        let mut activity = Arc::make_mut(&mut self.activity);
        for (var_name_ref, decision_var) in &model.variables {
            let cdom = &decision_var.domain;
            if cdom != &conjure_ast::Domain::BoolDomain {
                return Err(SolverError::ModelFeatureNotSupported(format!(
                    "variable {:?}: expected BoolDomain, found: {:?}",
                    decision_var, decision_var.domain
                )));
            }
            let sat_var = {
                let mut solver = self.solver.lock().map_err(|_| SolverError::Interrupted)?;
                solver.new_var()
            };
            let var_id = match var_name_ref {
                Name::MachineName(id) => *id,
                _ => {
                    return Err(SolverError::ModelInvalid(format!(
                        "Invalid variable name: {:?}",
                        var_name_ref
                    )))
                }
            };
            var_map.insert(var_id, sat_var);
            activity.insert(var_id, 0.0);
        }

        let constraints_vec: Vec<Expression> = model.get_constraints_vec();
        let cnf_clauses =
            handle_expr(Expression::And(Metadata { clean: false, etype: None }, constraints_vec))
                .map_err(|e| SolverError::ModelInvalid(format!("{:?}", e)))?;

        let mut solver = self.solver.lock().map_err(|_| SolverError::Interrupted)?;
        for clause in cnf_clauses {
            let solver_clause: Vec<Lit> = clause
                .iter()
                .map(|&lit| self.map_literal(lit))
                .collect();
            solver.add_clause(&solver_clause);
        }

        Ok(())
    }

    fn get_family(&self) -> SolverFamily {
        SolverFamily::SAT
    }
}

pub fn handle_expr(e: Expression) -> Result<Vec<Vec<i32>>, CNFError> {
    match e {
        Expression::And(_, exprs) => {
            let mut clauses = Vec::new();
            for expr in exprs {
                let sub_clauses = handle_expr(expr)?;
                clauses.extend(sub_clauses);
            }
            Ok(clauses)
        }
        Expression::Or(_, exprs) => {
            let mut clause = Vec::new();
            for expr in exprs {
                let lit = handle_lit(expr)?;
                clause.push(lit);
            }
            Ok(vec![clause])
        }
        Expression::Not(_, _) | Expression::Reference(_, _) => {
            let lit = handle_lit(e)?;
            Ok(vec![vec![lit]])
        }
        _ => Err(CNFError::UnexpectedExpression(e)),
    }
}

pub fn handle_lit(e: Expression) -> Result<i32, CNFError> {
    match e {
        Expression::Not(_, box expr) => {
            let lit = handle_lit(*expr)?;
            Ok(-lit)
        }
        Expression::Reference(_, name) => get_namevar_as_int(name),
        _ => Err(CNFError::UnexpectedLiteralExpression(e)),
    }
}

pub fn get_namevar_as_int(name: Name) -> Result<i32, CNFError> {
    match name {
        Name::MachineName(val) => Ok(val),
        _ => Err(CNFError::BadVariableType(name)),
    }
}

#[derive(Error, Debug)]
pub enum CNFError {
    #[error("Variable with name `{0}` not of right type")]
    BadVariableType(conjure_ast::Name),

    #[error("Unexpected Expression `{0}` inside Not(). Only Not(Reference) or Not(Not) allowed!")]
    UnexpectedExpressionInsideNot(conjure_ast::Expression),

    #[error("Unexpected Expression `{0}` as literal. Only Not() or Reference() allowed!")]
    UnexpectedLiteralExpression(conjure_ast::Expression),

    #[error("Unexpected Expression `{0}` found!")]
    UnexpectedExpression(conjure_ast::Expression),
}

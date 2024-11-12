use std::iter::Inspect;
use std::ptr::null;

// use sat_rs::sat_solvers::SatSolver;

use rustsat_minisat::core::Minisat;
use sat_rs::sat_tree;

use crate::solver::{SolveSuccess, SolverCallback, SolverFamily, SolverMutCallback};
use crate::Model as ConjureModel;

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

/// A [SolverAdaptor] for interacting with the SatSolver generic and the types thereof.

pub struct SAT {
    __non_constructable: private::Internal,
    model: Option<CNFModel>,
    inst: SatInstance,
}

impl private::Sealed for SAT {}

impl SAT {
    pub fn new() -> Self {
        SAT {
            __non_constructable: private::Internal,
            model: None,
            inst: SatInstance::new(),
        }
    }

    pub fn populate_instance(&mut self, vec_cnf: Vec<Vec<i32>>) -> () {
        sat_tree::conv_to_formula(&vec_cnf, &mut self.inst);
    }
}

impl Default for SAT {
    fn default() -> Self {
        SAT::new()
    }
}

impl SolverAdaptor for SAT {
    fn solve(
        &mut self,
        callback: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        // ToDo (ss504): this needs to be fixed after load_model
        let solver = Minisat::default(); // maybe change to use other solvers
        let inst = Ok(load_model());

        let res: Result<SolverResult, Error> = solver.solve().map().map_err(op);
        let unwrap: SolverResult = res.unwrap();

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
        // ToDo (ss504) use the sat_tree functions to create an instance (may not need to use model). Return Result<SatInstance, SolveError>

        let mut cnf_model: CNFModel = CNFModel::new();

        let model1: Result<CNFModel, SolverError> = CNFModel::from_conjure(model);
        // convert to SatInstance
        // self.model = model1

        // let res: Result<SatInstance, SolverError> = Result::err();
        // res
        None
    }

    fn get_family(&self) -> SolverFamily {
        SolverFamily::SAT
    }
}
// pub struct SatSolverStruct {
//     __non_constructable: private::Internal,
//     model: Option<CNFModel>,
// }

// impl private::Sealed for SatSolverStruct {}

// impl SatSolverStruct {
//     pub fn new() -> Self {
//         SatSolverStruct {
//             __non_constructable: private::Internal,
//             model: None,
//         }
//     }
// }

// impl Default for SatSolverStruct {
//     fn default() -> Self {
//         SatSolverStruct::new()
//     }
// }

// impl Default for SatSolver {
//     fn default() -> Self {
//         SatSolver::new(null)
//     }
// }

// impl SolverAdaptor for SatSolverStruct {
//     fn solve(
//         &mut self,
//         callback: SolverCallback,
//         _: private::Internal,
//     ) -> Result<SolveSuccess, SolverError> {
//         Err(OpNotSupported("solve".to_owned()))
//     }

//     fn solve_mut(
//         &mut self,
//         callback: SolverMutCallback,
//         _: private::Internal,
//     ) -> Result<SolveSuccess, SolverError> {
//         Err(OpNotSupported("solve_mut".to_owned()))
//     }

//     fn load_model(&mut self, model: ConjureModel, _: private::Internal) -> Result<(), SolverError> {
//         self.model = Some(CNFModel::from_conjure(model)?);
//         Ok(())
//     }

//     fn get_family(&self) -> SolverFamily {
//         SolverFamily::SAT
//     }
// }

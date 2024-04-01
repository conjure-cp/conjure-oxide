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

/// A [SolverAdaptor] for interacting with the Kissat SAT solver.
pub struct Kissat {
    __non_constructable: private::Internal,
    model: Option<CNFModel>,
}

impl private::Sealed for Kissat {}

impl Kissat {
    pub fn new() -> Self {
        Kissat {
            __non_constructable: private::Internal,
            model: None,
        }
    }
}

impl Default for Kissat {
    fn default() -> Self {
        Kissat::new()
    }
}

impl SolverAdaptor for Kissat {
    fn solve(
        &mut self,
        callback: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(OpNotImplemented("solve(): todo!".to_owned()))
    }

    fn solve_mut(
        &mut self,
        callback: SolverMutCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(OpNotSupported("solve_mut".to_owned()))
    }

    fn load_model(&mut self, model: ConjureModel, _: private::Internal) -> Result<(), SolverError> {
        self.model = Some(CNFModel::from_conjure(model)?);
        Ok(())
    }

    fn get_family(&self) -> SolverFamily {
        SolverFamily::SAT
    }
}

use super::super::model_modifier::NotModifiable;
use super::super::private;
use super::super::stats::NoStats;
use super::super::SearchComplete::*;
use super::super::SearchIncomplete::*;
use super::super::SearchStatus::*;
use super::super::SolveSuccess;
use super::super::SolverAdaptor;
use super::super::SolverError;
use super::super::SolverError::*;
use super::sat_common::CNFModel;
use crate::solver::SolverMutCallback;
use crate::solver::{states, SolverCallback};
use crate::Model as ConjureModel;

/// A [SolverAdaptor] for interacting with the Kissat SAT solver.
pub struct Kissat {
    __non_constructable: private::Internal,
}

impl private::Sealed for Kissat {}

impl SolverAdaptor for Kissat {
    type Model = CNFModel;

    type Solution = ();

    type Modifier = NotModifiable;

    fn new() -> Self {
        Kissat {
            __non_constructable: private::Internal,
        }
    }

    fn solve(
        &mut self,
        model: Self::Model,
        callback: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(OpNotImplemented("solve(): todo!".to_owned()))
    }

    fn solve_mut(
        &mut self,
        model: Self::Model,
        callback: SolverMutCallback<Self>,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(OpNotSupported("solve_mut".to_owned()))
    }

    fn load_model(
        &mut self,
        model: ConjureModel,
        _: private::Internal,
    ) -> Result<Self::Model, SolverError> {
        CNFModel::from_conjure(model)
    }
}

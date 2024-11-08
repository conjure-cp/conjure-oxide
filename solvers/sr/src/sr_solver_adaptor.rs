use conjure_oxide::conjure_core::solver::{SolverAdaptor, SolveError};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use conjure_oxide::conjure_core::essence::model::EssenceModel;

pub struct SRSolverAdaptor; 

impl SolverAdaptor for SRSolverAdaptor {
   
   //TODO
    fn solve(
        &mut self,
        callback: SolverCallback,
        _: Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(SolverError::from("TODO"))
    }

    //TODO
    fn solve_mut(
        &mut self,
        callback: SolverMutCallback,
        _: Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(SolverError::from("TODO"))
    }

    //TODO
    fn load_model(
        &mut self,
        model: Model,
        _: Internal,
    ) -> Result<(), SolverError> {
        Err(SolverError::from("TODO"))
    }
    
    //TODO
    fn get_family(&self) -> SolverFamily {
        SolverFamily::default() 
    }

}

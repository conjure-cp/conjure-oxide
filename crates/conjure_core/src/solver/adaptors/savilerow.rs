use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::process::Command;

use minion_rs::get_from_table;

use crate::solver::private::Internal;
use crate::Model;
use crate::ast as conjure_ast;
use crate::solver::SearchStatus::Complete;
use crate::solver::{SearchComplete, SearchStatus, SolverCallback};
use crate::solver::SolverFamily;
use crate::solver::SolverMutCallback;
use crate::stats::SolverStats;
use crate::Model as ConjureModel;

use super::super::private;
use super::super::SearchComplete::*;
use super::super::SearchIncomplete::*;
use super::super::SolveSuccess;
use super::super::SolverAdaptor;
use super::super::SolverError;
use super::super::SolverError::*;

/// A [SolverAdaptor] for interacting with SavileRow.

pub struct SavileRow{
    essence_prime_file: Option<PathBuf>,
    solutions_dir: Option<PathBuf>,
}

impl private::Sealed for SavileRow {}

impl SavileRow {
    pub fn new() -> Self {
        SavileRow{
            essence_prime_file: None,
            solutions_dir: None
        }
    }
}


impl SolverAdaptor for SavileRow {
    
    #[allow(clippy::unwrap_used)]
    fn solve(
        &mut self,
        callback: SolverCallback,
        _: Internal,
    ) -> Result<SolveSuccess, SolverError> {
        //ensures that the file is loaded (returns error if not)
        let essence_prime_file = self
            .essence_prime_file
            .as_ref()
            .ok_or(SolverError::ModelInvalid(("No model loaded".to_owned())))?;

        //prepare temporary directory for solutions
        let tmp_dir =std::env::temp_dir().join("savilerow_solutions");
        std::fs::create_dir_all(&tmp_dir).map_err(|e| SolverError::Runtime((e.to_string())))?;

        /*Not sure if this is how you call the external command for SavileRow
        But this passes in the model path and the solutions directory as args*/
        let output = Command::new("savilerow")
            .arg(essence_prime_file)
            .arg("--solutions-dir")
            .arg(&tmp_dir)
            .output()
            .map_err(|e| SolverError::Runtime(e.to_string()))?;

        /*if successful, updates solutions_dir and returns SolverSuccess with stats
        if not, returns a runtime error*/

        if !output.status.success() {
            return Err(SolverError::Runtime(("Savile Row Error".to_owned())));
        }

        //need to implement logic for if status is InComplete and NoSolutions
        let mut status = Complete(HasSolutions);

        self.solutions_dir = Some(tmp_dir);
        Ok(SolveSuccess {
            stats: get_solver_stats(),
            status,
        })
    }

    //Not implemented yet
    fn solve_mut(
        &mut self,
        callback: SolverMutCallback,
        _: Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(OpNotImplemented(("solve_mut".into())))
    }

    /*Transforms the provided model to Essence Prime and stores the path
    in the essence_prime_file field*/
    fn load_model(
        &mut self,
        model: Model,
        _: Internal,
    ) -> Result<(), SolverError> {
        let tmp_dir = std::env::temp_dir();
        let essence_prime_path = tmp_dir.join("model.eprime");

        //not implemented yet
        transform_to_essence_prime(&model, &essence_prime_path)?;

        self.essence_prime_file = Some(essence_prime_path);
        Ok(())
    }
    
    //Added SavileRow enum to return the correct family
    fn get_family(&self) -> SolverFamily {
        SolverFamily::SavileRow
    }

    //self-explanatory
    fn get_name(&self) -> Option<String> {
        Some("SavileRow".to_owned())
    }

}

//Function to transform a model to Essence Prime
fn transform_to_essence_prime(
    model: &ConjureModel,
    output_path: &PathBuf,
) -> Result<(), SolverError> {
    //transformation logic goes here
    Ok(())
}

/*Meant to get the solver statistics - taken directly from minion.rs
Need explanation here*/
#[allow(clippy::unwrap_used)]
fn get_solver_stats() -> SolverStats {
    SolverStats {
        nodes: get_from_table("Nodes".into()).map(|x| x.parse::<u64>().unwrap()),
        ..Default::default()
    }
}

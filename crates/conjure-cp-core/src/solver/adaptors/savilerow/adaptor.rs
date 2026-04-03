//! Solver adaptor for Savile Row, called as an external process.

use std::collections::HashMap;
use std::io::Write;
use std::process::Command;

use tempfile::tempdir;

use crate::Model;
use crate::ast::{Literal, Name};
use crate::settings::SolverFamily;
use crate::solver::{
    SearchComplete, SearchStatus, SolveSuccess, SolverAdaptor, SolverCallback, SolverError,
    SolverMutCallback, private,
};
use crate::stats::SolverStats;

/// Solver adaptor for Savile Row.
///
/// Savile Row is called as an external process. The model is written to a
/// temporary file, Savile Row is invoked, and its output is parsed back
/// into Conjure Oxide's solution format.
pub struct SavileRow {
    model: Option<Model>,
}

impl SavileRow {
    pub fn new() -> Self {
        SavileRow { model: None }
    }
}

impl Default for SavileRow {
    fn default() -> Self {
        Self::new()
    }
}

impl private::Sealed for SavileRow {}

impl SolverAdaptor for SavileRow {
    fn load_model(
        &mut self,
        model: Model,
        _: private::Internal,
    ) -> Result<(), SolverError> {
        self.model = Some(model);
        Ok(())
    }

    fn solve(
        &mut self,
        callback: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        // Get the model we stored in load_model
        let model = self.model.as_ref().ok_or(SolverError::ModelInvalid(
            "No model loaded".into(),
        ))?;

        // Prepend the Essence' header to the model's Display output
        // Model::Display already produces variable declarations and constraints
        // in Essence' format - we just need the language header at the top
        let model_str = format!("language ESSENCE' 1.0\n\n{}", model);

        // Create a temporary directory - this gets automatically deleted
        // when it goes out of scope at the end of this function
        let tmp_dir = tempdir().map_err(|e| {
            SolverError::Runtime(format!("Failed to create temp directory: {e}"))
        })?;

        // Write the model string to a file inside the temp directory
        let input_path = tmp_dir.path().join("model.eprime");
        std::fs::write(&input_path, &model_str).map_err(|e| {
            SolverError::Runtime(format!("Failed to write model file: {e}"))
        })?;

        Err(SolverError::OpNotImplemented(
            "Savile Row solve not yet implemented".into(),
        ))
    }

    fn solve_mut(
        &mut self,
        _callback: SolverMutCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(SolverError::OpNotSupported(
            "Savile Row does not support incremental solving".into(),
        ))
    }

    fn get_family(&self) -> SolverFamily {
        SolverFamily::SavileRow
    }

    fn get_name(&self) -> &'static str {
        "SavileRow"
    }

    fn write_solver_input_file(
        &self,
        writer: &mut Box<dyn Write>,
    ) -> Result<(), std::io::Error> {
        writer.write_all(b"// Savile Row input file (not yet implemented)\n")
    }
}

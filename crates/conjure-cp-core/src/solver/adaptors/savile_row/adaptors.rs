//! Solver adaptor for Savile Row, called as an external process.
//Test commit comment
use std::io::Write;

use crate::Model;
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
        _callback: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
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

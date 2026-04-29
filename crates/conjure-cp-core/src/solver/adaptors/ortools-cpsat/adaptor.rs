use std::io::Write;

use crate::settings::SolverFamily;
use crate::solver::private;
use crate::solver::{SolveSuccess, SolverAdaptor, SolverCallback, SolverError, SolverMutCallback};
use super::proto::CpModelProto;
use crate::Model;

use super::convs::model_to_cp_sat;

pub struct OrToolsCpSat {
    __non_constructable: private::Internal,
    model: Option<CpModelProto>,
}

impl private::Sealed for OrToolsCpSat {}

impl OrToolsCpSat {
    pub fn new() -> Self {
        Self {
            __non_constructable: private::Internal,
            model: None,
        }
    }
}

impl Default for OrToolsCpSat {
    fn default() -> Self {
        Self::new()
    }
}

impl SolverAdaptor for OrToolsCpSat {
    fn solve(
        &mut self,
        _: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(SolverError::OpNotImplemented(
            "ortools-cpsat solve".to_owned(),
        ))
    }

    fn solve_mut(
        &mut self,
        _: SolverMutCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(SolverError::OpNotSupported(
            "ortools-cpsat solve_mut".to_owned(),
        ))
    }

    fn load_model(&mut self, model: Model, _: private::Internal) -> Result<(), SolverError> {
        self.model = Some(model_to_cp_sat(model)?);
        Ok(())
    }

    fn get_family(&self) -> SolverFamily {
        SolverFamily::OrToolsCpSat
    }

    fn get_name(&self) -> &'static str {
        "ortools-cpsat"
    }

    fn write_solver_input_file(&self, writer: &mut Box<dyn Write>) -> Result<(), std::io::Error> {
        writeln!(writer, "# Conjure Oxide OR-Tools CP-SAT backend scaffold")?;
        writeln!(
            writer,
            "# solving is not implemented yet; this is a debug placeholder"
        )?;

        if let Some(model) = &self.model {
            writeln!(writer, "{model:#?}")?;
        }
        Ok(())
    }
}

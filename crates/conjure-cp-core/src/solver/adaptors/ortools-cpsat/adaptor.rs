use std::io::Write;

use prost::Message;

use crate::Model;
use crate::ast::{Literal, Name};
use crate::settings::SolverFamily;
use crate::solver::SearchComplete::{HasSolutions, NoSolutions};
use crate::solver::SearchStatus::{Complete, Incomplete};
use crate::solver::private;
use crate::solver::{
    SearchIncomplete, SolveSuccess, SolverAdaptor, SolverCallback, SolverError, SolverMutCallback,
};
use crate::stats::SolverStats;

use super::convs::{model_to_cp_sat, response_to_solution, SolutionVar};
use super::ffi;
use super::proto::{CpModelProto, CpSolverResponse, CpSolverStatus};

pub struct OrToolsCpSat {
    __non_constructable: private::Internal,
    model: Option<CpModelProto>,
    solution_vars: Vec<SolutionVar>,
}

impl private::Sealed for OrToolsCpSat {}

impl OrToolsCpSat {
    pub const IS_AVAILABLE: bool = true;

    pub fn new() -> Self {
        Self {
            __non_constructable: private::Internal,
            model: None,
            solution_vars: Vec::new(),
        }
    }
}

impl Default for OrToolsCpSat {
    fn default() -> Self {
        Self::new()
    }
}

impl SolverAdaptor for OrToolsCpSat {

    fn load_model(&mut self, model: Model, _: private::Internal) -> Result<(), SolverError> {
        let (cp_model, solution_vars) = model_to_cp_sat(model)?;
        self.model = Some(cp_model);
        self.solution_vars = solution_vars;
        Ok(())
    }

    fn solve(
        &mut self,
        callback: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| SolverError::Runtime("No OR-Tools model loaded".to_owned()))?;

        let model_bytes = model.encode_to_vec();
        let response_bytes = ffi::solve_wrapper(&model_bytes);
        if response_bytes.is_empty() {
            return Err(SolverError::Runtime(
                "OR-Tools wrapper returned an empty response".to_owned(),
            ));
        }

let mut offset = 0;
        let mut responses = Vec::new();
        let bytes = response_bytes.as_slice();
        
        while offset < bytes.len() {
            if offset + 4 > bytes.len() {
                break;
            }
            let len = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
            offset += 4;
            
            if len > 100_000_000 { 
                return Err(SolverError::Runtime(format!(
                    "FATAL: OR-Tools response declared an impossible length ({} bytes) at offset {}. Corrupted stream.", 
                    len, offset - 4
                )));
            }

            if offset + len > bytes.len() {
                break; 
            }
            let slice = &bytes[offset..offset + len];
            responses.push(CpSolverResponse::decode(slice).map_err(|err| {
                SolverError::Runtime(format!("Failed to decode OR-Tools response: {err}"))
            })?);
            offset += len;
        }

        let final_response = responses.pop().ok_or_else(|| {
            SolverError::Runtime("No final response from OR-Tools".to_owned())
        })?;

        let status = CpSolverStatus::try_from(final_response.status).map_err(|_| {
            SolverError::Runtime(format!("Unknown OR-Tools solver status: {}", final_response.status))
        })?;

        let stats = SolverStats {
            solver_time_s: final_response.wall_time,
            nodes: u64::try_from(final_response.num_branches).ok(),
            satisfiable: Some(matches!(
                status,
                CpSolverStatus::Feasible | CpSolverStatus::Optimal
            )),
            sat_vars: u64::try_from(final_response.num_booleans).ok(),
            ..Default::default()
        };

        match status {
            CpSolverStatus::Optimal | CpSolverStatus::Feasible => {
                for response in responses {
                    let solution = response_to_solution(&response, &self.solution_vars)?;
                    if !callback(solution) {
                        return Ok(SolveSuccess {
                            stats,
                            status: Incomplete(SearchIncomplete::UserTerminated),
                        });
                    }
                }

                Ok(SolveSuccess {
                    stats,
                    status: Complete(HasSolutions),
                })
            }
            CpSolverStatus::Infeasible => Ok(SolveSuccess {
                stats,
                status: Complete(NoSolutions),
            }),
            CpSolverStatus::ModelInvalid => Err(SolverError::ModelInvalid(
                if final_response.solution_info.is_empty() {
                    "OR-Tools reported MODEL_INVALID".to_owned()
                } else {
                    final_response.solution_info
                },
            )),
            CpSolverStatus::Unknown => Err(SolverError::Runtime(if final_response.solution_info.is_empty()
            {
                "OR-Tools returned UNKNOWN".to_owned()
            } else {
                format!("OR-Tools returned UNKNOWN: {}", final_response.solution_info)
            })),
        }
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

    fn get_family(&self) -> SolverFamily {
        SolverFamily::OrToolsCpSat
    }

    fn get_name(&self) -> &'static str {
        "ortools-cpsat"
    }

    fn write_solver_input_file(&self, writer: &mut Box<dyn Write>) -> Result<(), std::io::Error> {
        writeln!(writer, "# Conjure Oxide OR-Tools CP-SAT backend scaffold")?;

        if let Some(model) = &self.model {
            writeln!(writer, "{model:#?}")?;
        }
        Ok(())
    }
}
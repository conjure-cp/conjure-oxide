use serde::Serialize;
use serde_with::skip_serializing_none;

use crate::solver::SolverFamily;

#[skip_serializing_none]
#[derive(Default, Serialize, Clone)]
#[allow(dead_code)]
pub struct SolverStats {
    // Wall time as measured by Conjure Oxide.
    // This is set by Solver, not SolverAdaptor
    pub conjure_solver_wall_time_s: f64,

    pub solver_family: Option<SolverFamily>,

    // NOTE (niklasdewally): these fields are copied from the list in Savile Row
    pub nodes: Option<u64>,
    pub satisfiable: Option<bool>,
    pub sat_vars: Option<u64>,
    pub sat_clauses: Option<u64>,
}

impl SolverStats {
    // If the given stats object exists, add the wall time value.
    // Otherwise create a new stats object containing the wall time value.
    pub fn with_timings(self, wall_time_s: f64) -> SolverStats {
        SolverStats {
            conjure_solver_wall_time_s: wall_time_s,
            ..self.clone()
        }
    }
}

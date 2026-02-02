use schemars::JsonSchema;
use serde::Serialize;
use serde_with::skip_serializing_none;

use crate::solver::SolverFamily;

#[skip_serializing_none]
#[derive(Default, Serialize, Clone, JsonSchema, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
// Statistics for a run of a solver.
pub struct SolverStats {
    #[serde(rename = "conjureSolverWallTime_s")]
    /// Wall time as measured by Conjure-Oxide (not the solver).
    pub conjure_solver_wall_time_s: f64,

    // This is set by Solver, not SolverAdaptor
    /// The solver family used for this run.
    pub solver_family: Option<SolverFamily>,

    /// The solver adaptor used for this run.
    pub solver_adaptor: Option<String>,

    // NOTE (niklasdewally): these fields are copied from the list in Savile Row
    pub nodes: Option<u64>,
    pub satisfiable: Option<bool>,
    pub sat_vars: Option<u64>,
    pub sat_clauses: Option<u64>,
}

impl SolverStats {
    // Adds the conjure_solver_wall_time_s to the stats.
    pub fn with_timings(self, wall_time_s: f64) -> SolverStats {
        SolverStats {
            conjure_solver_wall_time_s: wall_time_s,
            ..self
        }
    }
}

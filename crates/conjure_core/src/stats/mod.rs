mod solver_stats;

use schemars::JsonSchema;
use serde::Serialize;
use serde_with::skip_serializing_none;
pub use solver_stats::SolverStats;

#[allow(dead_code)]
#[skip_serializing_none]
#[derive(Default, Serialize, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub solver_runs: Vec<SolverStats>,
    pub rewriter_run_time : Option<std::time::Duration>, 
}

impl Stats {
    pub fn new() -> Stats {
        Default::default()
    }

    pub fn add_solver_run(&mut self, solver_stats: SolverStats) {
        self.solver_runs.push(solver_stats);
    }
}

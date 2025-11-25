mod rewriter_stats;
mod solver_stats;

pub use rewriter_stats::RewriterStats;
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
    pub rewriter_runs: Vec<RewriterStats>,
}

impl Stats {
    pub fn new() -> Stats {
        Default::default()
    }

    pub fn add_solver_run(&mut self, solver_stats: SolverStats) {
        self.solver_runs.push(solver_stats);
    }

    pub fn add_rewriter_run(&mut self, rewriter_stats: RewriterStats) {
        self.rewriter_runs.push(rewriter_stats);
    }
}

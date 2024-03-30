mod solver_stats;

use serde::Serialize;
pub use solver_stats::SolverStats;

#[allow(dead_code)]
#[derive(Default, Serialize, Clone)]
pub struct Stats {
    pub solve_wall_time_s: Option<f64>,
    pub solver_runs: Vec<SolverStats>,
}

impl Stats {
    pub fn new() -> Stats {
        Default::default()
    }

    pub fn add_solver_run(&mut self, solver_stats: SolverStats) {
        self.solver_runs.push(solver_stats);
    }
}

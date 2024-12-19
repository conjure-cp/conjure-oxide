use anyhow::{Error, Result};
use rustsat::instances::SatInstance;
use rustsat::solvers::{Solve, SolverResult};
use rustsat_minisat::simp::Minisat;

pub trait Solver {
    fn solve(&self, instance: &SatInstance) -> Result<SolverResult>;
}

pub struct SatSolver<SolverType> {
    solver: SolverType,
}

impl<SolverType: Solver> SatSolver<SolverType> {
    pub fn new(solver: SolverType) -> Self {
        SatSolver { solver }
    }

    pub fn solve(&self, inst: SatInstance) -> Result<SolverResult> {
        self.solver.solve(&inst)
    }

    pub fn solver_instance(&self) -> &SolverType {
        &self.solver
    }
}

impl Solver for Minisat {
    fn solve(&self, instance: &SatInstance) -> Result<SolverResult, Error> {
        let res: Result<SolverResult, Error> = self.solve(instance);
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustsat::instances::SatInstance;
    use std::collections::HashMap;

    #[test]
    fn test_minisat_solver_satisfiable() {
        let mut instance = SatInstance::new();
        // Example: (1 OR -2) AND (-1 OR 2)
        let clause1 = vec![1, -2];
        let clause2 = vec![-1, 2];
        let mut var_map = HashMap::new();
        crate::conversions::conv_to_clause(&clause1, &mut instance, &mut var_map).unwrap();
        crate::conversions::conv_to_clause(&clause2, &mut instance, &mut var_map).unwrap();

        let solver = Minisat::default();
        let sat_solver = SatSolver::new(solver);
        let result = sat_solver.solve(instance).unwrap();

        assert_eq!(result, SolverResult::Sat);
    }

    #[test]
    fn test_minisat_solver_unsatisfiable() {
        let mut instance = SatInstance::new();
        // Example: (1) AND (-1)
        let clause1 = vec![1];
        let clause2 = vec![-1];
        let mut var_map = HashMap::new();
        crate::conversions::conv_to_clause(&clause1, &mut instance, &mut var_map).unwrap();
        crate::conversions::conv_to_clause(&clause2, &mut instance, &mut var_map).unwrap();

        let solver = Minisat::default();
        let sat_solver = SatSolver::new(solver);
        let result = sat_solver.solve(instance).unwrap();

        assert_eq!(result, SolverResult::Unsat);
    }
}

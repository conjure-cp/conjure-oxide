use rustsat::instances::SatInstance;
use rustsat_minisat::simp::Minisat;
use rustsat::solvers::SolverResult;
use anyhow::{anyhow, Result};

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

    pub fn solve(&self, inst: &SatInstance) -> Result<SolverResult> {
        self.solver.solve(inst)
    }

    pub fn solver_instance(&self) -> &SolverType {
        &self.solver
    }
}

impl Solver for Minisat {
    fn solve(&self, instance: &SatInstance) -> Result<SolverResult> {
        self.solve_instance(instance).map_err(|e| anyhow!("Minisat error: {}", e))
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
        crate::sat_tree::conv_to_clause(&clause1, &mut instance, &mut var_map).unwrap();
        crate::sat_tree::conv_to_clause(&clause2, &mut instance, &mut var_map).unwrap();

        let solver = Minisat::default();
        let sat_solver = SatSolver::new(solver);
        let result = sat_solver.solve(&instance).unwrap();

        assert_eq!(result, SolverResult::Sat);
    }

    #[test]
    fn test_minisat_solver_unsatisfiable() {
        let mut instance = SatInstance::new();
        // Example: (1) AND (-1)
        let clause1 = vec![1];
        let clause2 = vec![-1];
        let mut var_map = HashMap::new();
        crate::sat_tree::conv_to_clause(&clause1, &mut instance, &mut var_map).unwrap();
        crate::sat_tree::conv_to_clause(&clause2, &mut instance, &mut var_map).unwrap();

        let solver = Minisat::default();
        let sat_solver = SatSolver::new(solver);
        let result = sat_solver.solve(&instance).unwrap();

        assert_eq!(result, SolverResult::Unsat);
    }
}


// use rustsat::instances::SatInstance;
// use rustsat_minisat;

// pub trait Solver {
//     fn solve(&self, instance: &SatInstance) -> bool;
// }

// pub struct SatSolver<SolverType> {
//     inst: SatInstance,
//     solver: SolverType,
// }

// impl<SolverType: Solver> SatSolver<SolverType> {
//     // Constructor to create a new SatSolverInst
//     // pub fn new(inst: &SatInstance, solver: SolverType) -> Self {
//     //     SatSolver { inst, solver }
//     // }

//     pub fn new(solver: SolverType) -> Self {
//         SatSolver {
//             inst: SatInstance::new(),
//             solver,
//         }
//     }

//     // Method to solve the SAT instance using the specified solver
//     pub fn solve(&self) -> bool {
//         self.solver.solve(&self.inst)
//     }
// }

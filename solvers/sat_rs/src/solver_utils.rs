use crate::sat_solvers::SatSolver;
use crate::sat_tree::conv_to_formula;
use anyhow::{Error, Result};
use rustsat::instances::SatInstance;
use rustsat::solvers::SolverResult;
use rustsat_minisat::simp::Minisat;

pub fn initialize_solver(vec_problem: &Vec<Vec<i32>>) -> Result<(SatSolver<Minisat>, SatInstance)> {
    let mut inst: SatInstance = SatInstance::new();
    conv_to_formula(vec_problem, &mut inst)?;
    let minisat_solver = Minisat::default();
    let sat_solver = SatSolver::new(minisat_solver);
    Ok((sat_solver, inst))
}

pub fn solve_problem(
    sat_solver: &SatSolver<Minisat>,
    instance: SatInstance,
) -> Result<SolverResult, Error> {
    let res = sat_solver.solve(instance)?;
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustsat::{instances::SatInstance, solvers::SolverResult};
    use std::collections::HashMap;

    #[test]
    fn test_initialize_and_solve_satisfiable() {
        let problem = vec![
            vec![1, 2, -3],
            vec![-1, 3],
            vec![2, -3],
            vec![-2, 3],
            vec![1, -2],
        ];

        let (sat_solver, inst) = initialize_solver(&problem).unwrap();
        let result = solve_problem(&sat_solver, inst).unwrap();

        assert_eq!(result, SolverResult::Sat);
    }

    #[test]
    fn test_initialize_and_solve_unsatisfiable() {
        let problem = vec![vec![1], vec![-1]];

        let (sat_solver, inst) = initialize_solver(&problem).unwrap();
        let result = solve_problem(&sat_solver, inst).unwrap();

        assert_eq!(result, SolverResult::Unsat);
    }
}

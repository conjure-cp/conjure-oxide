use anyhow::Error;
use rustsat::solvers::SolverResult;
use sat_rs::sat_solvers::SatSolver;
use sat_rs::solver_utils::{initialize_solver, solve_problem};

fn main() -> Result<(), Error> {
    let v1: Vec<i32> = vec![1, 2, -3];
    let v2: Vec<i32> = vec![-1, 3];
    let v3: Vec<i32> = vec![2, -3];
    let v4: Vec<i32> = vec![-2, 3];
    let v5: Vec<i32> = vec![1, -2];

    let vec_problem: Vec<Vec<i32>> = vec![v1, v2, v3, v4, v5];

    // Initializes SAT Solver and SAT Instance
    let (sat_solver, inst) = initialize_solver(&vec_problem)?;

    // Solves the Problem
    let res: SolverResult = solve_problem(&sat_solver, &inst)?;

    // Solver Result
    match res {
        SolverResult::Sat => {
            println!("SATISFIABLE");
        }
        SolverResult::Unsat => println!("UNSATISFIABLE"),
    }

    Ok(())
}

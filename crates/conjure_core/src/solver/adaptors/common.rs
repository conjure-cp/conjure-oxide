use crate::solver::SolverFamily;

use rustsat::instances::SatInstance;

use sat_rs::{sat_solvers::SatSolverInst, sat_tree};

// use rustsat_minisat: 

trait SolverAdaptor {
    fn prep_dat();

    fn get_soln();

    fn print_soln() -> ();
}

impl SolverAdaptor for SatSolverInst<SolverType: > {
    fn prep_dat(vec_problem: &Vec<Vec<i16>>, inst_in_use: &mut SatInstance) -> () {
        // todo!()
        sat_tree::conv_to_formula(&vec_problem, &mut self.inst);
    }
    
    fn get_soln() -> Result<bool, String>{
        // todo!()
        self.solver.solve()
    }
    
    fn print_soln() {
        // todo!()
        match problem.solve() {
            Ok(true) => {
                println!("SATISFIABLE");
                println!("a = {}", problem.get_assignment(a).unwrap());
                println!("b = {}", problem.get_assignment(b).unwrap());
            }
            Ok(false) => println!("UNSATISFIABLE"),
            Err(e) => println!("Error during solving: {:?}", e),
        }
    }

    
}

struct MinionAdaptor {
}
use crate::solver::SolverFamily;

use rustsat::instances::SatInstance;

use sat_rs::{sat_solvers::SatSolverInst, sat_tree};

trait SolverAdaptor {
    fn prep_dat();

    fn get_soln();
}

struct SatAdaptor {
    // unimplemented
    adaptor_instance: SatInstance,
    solver_instance: SatSolverInst
}

impl SolverAdaptor for SatSolverInst {
    fn prep_dat(vec_problem: &Vec<Vec<i16>>, inst_in_use: &mut SatInstance) -> () {
        // todo!()
        sat_tree::conv_to_formula(&vec_problem, &mut inst);
    }

    fn get_soln() {
        todo!()
    }
}

struct MinionAdaptor {
}
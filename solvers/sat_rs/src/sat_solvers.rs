use rustsat::instances::SatInstance;
use rustsat_minisat;

pub trait Solver {
    fn solve(&self, instance: &SatInstance) -> bool;
}

pub struct SatSolver<SolverType> {
    inst: SatInstance,
    solver: SolverType,
}

impl<SolverType: Solver> SatSolver<SolverType> {
    // Constructor to create a new SatSolverInst
    // pub fn new(inst: &SatInstance, solver: SolverType) -> Self {
    //     SatSolver { inst, solver }
    // }

    pub fn new(solver: SolverType) -> Self {
        SatSolver {
            inst: SatInstance::new(),
            solver,
        }
    }

    // Method to solve the SAT instance using the specified solver
    pub fn solve(&self) -> bool {
        self.solver.solve(&self.inst)
    }
}

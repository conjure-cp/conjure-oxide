use rustsat::instances::SatInstance;
use rustsat_minisat;

pub struct SatSolverInst<SolverType: SatSolver> {
    inst: SatInstance,
    solver: SolverType
}
use rustsat::instances::SatInstance;
use rustsat_minisat;

pub trait SatSolverInst{
    fn give(inst: SatInstance);
    fn solve();
}

impl SatSolverInst for rustsat_minisat {
    fn give(inst: SatInstance) {
        todo!() 
    }
    fn solve() {
        todo!()
    }
}
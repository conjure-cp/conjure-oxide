use rustsat::instances::SatInstance;
use rustsat_minisat;

trait SatSolverInst{
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
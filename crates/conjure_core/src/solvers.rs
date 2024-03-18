use strum_macros::Display;
use strum_macros::{EnumIter, EnumString};

/// All supported solvers.
#[derive(Debug, EnumString, EnumIter, Display)]
pub enum Solver {
    Minion,
    KissSAT,
}

#[derive(Debug, EnumString, EnumIter, Display)]
pub enum SolverFamily {
    SAT,
    Minion,
}

impl Solver {
    pub fn family(&self) -> SolverFamily {
        match self {
            Solver::Minion => SolverFamily::Minion,
            Solver::KissSAT => SolverFamily::SAT,
        }
    }
}

impl SolverFamily {
    pub fn members(&self) -> &[Solver] {
        match self {
            SolverFamily::Minion => &[Solver::Minion],
            SolverFamily::SAT => &[Solver::KissSAT],
        }
    }
}
use strum_macros::Display;
use strum_macros::{EnumIter, EnumString};

/// All supported solvers.
#[derive(Debug, EnumString, EnumIter, Display, PartialEq, Eq, Hash, Clone, Copy)]
pub enum SolverName {
    Minion,
    KissSAT,
}

#[derive(Debug, EnumString, EnumIter, Display, PartialEq, Eq, Hash, Clone, Copy)]
pub enum SolverFamily {
    SAT,
    Minion,
}

impl SolverName {
    pub fn family(&self) -> SolverFamily {
        match self {
            SolverName::Minion => SolverFamily::Minion,
            SolverName::KissSAT => SolverFamily::SAT,
        }
    }
}

impl SolverFamily {
    pub fn solvers(&self) -> &[SolverName] {
        match self {
            SolverFamily::Minion => &[SolverName::Minion],
            SolverFamily::SAT => &[SolverName::KissSAT],
        }
    }
}
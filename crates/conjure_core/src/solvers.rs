use strum_macros::Display;
use strum_macros::{EnumIter, EnumString};

/// All supported solvers.
#[derive(Debug, EnumString, EnumIter, Display)]
pub enum Solver {
    Minion,
    KissSAT,
}

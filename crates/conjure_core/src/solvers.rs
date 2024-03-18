use strum_macros::Display;
use strum_macros::{EnumIter, EnumString};

#[derive(Debug, EnumString, EnumIter, Display, PartialEq, Eq, Hash, Clone, Copy)]
#[derive(Default)]
pub enum SolverFamily {
    SAT,
    #[default]
    Minion,
}

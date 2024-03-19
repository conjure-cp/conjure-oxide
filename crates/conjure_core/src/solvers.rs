use serde::{Deserialize, Serialize};
use strum_macros::Display;
use strum_macros::{EnumIter, EnumString};

#[derive(Debug, EnumString, EnumIter, Display, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize)]
pub enum SolverFamily {
    SAT,
    Minion,
}

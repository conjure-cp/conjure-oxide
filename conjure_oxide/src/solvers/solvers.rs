//! All supported solvers.

use strum_macros::Display;
use strum_macros::{EnumIter, EnumString};

/// All supported solvers.
///
/// This enum implements, Display and Iter, so can be used as a string, or iterated over.
#[derive(Debug, EnumString, EnumIter, Display)]
pub enum Solver {
    Minion,
    KissSAT,
}

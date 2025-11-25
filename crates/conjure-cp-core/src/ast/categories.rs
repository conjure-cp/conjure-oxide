use std::fmt::Display;

use serde::{Deserialize, Serialize};

/// The *category* of a term describes the kind of symbols it contains.
///
/// Categories have a strict order: constant < parameter < quantifying variable < decision
/// variable.
#[derive(Copy, Clone, Debug, PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Category {
    /// This term does not have a category in isolation - e.g. a record field declaration
    Bottom = 0,
    /// This term contains constants and lettings
    Constant = 1,
    /// This term contains parameters / givens
    Parameter = 2,
    /// This term contains quantified variables / induction variables
    Quantified = 3,
    /// This term contains decision variables
    Decision = 4,
}

impl Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Category::Bottom => write!(f, "_|_"),
            Category::Constant => write!(f, "constant"),
            Category::Parameter => write!(f, "parameter"),
            Category::Quantified => write!(f, "quantified"),
            Category::Decision => write!(f, "decision"),
        }
    }
}

/// A type with a [`Category`]
pub trait CategoryOf {
    /// Gets the [`Category`] of a term.
    fn category_of(&self) -> Category;
}

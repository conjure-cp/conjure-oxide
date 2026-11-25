use serde::{Deserialize, Serialize};

use super::Expression;

/// Whether an objective is minimised or maximised.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OptimiseDirection {
    Minimising,
    Maximising,
}

/// A single-objective optimisation statement.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Objective {
    pub direction: OptimiseDirection,
    pub expression: Expression,
}

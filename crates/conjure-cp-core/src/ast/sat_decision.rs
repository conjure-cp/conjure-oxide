//! Semantic SAT encoding decisions. Clauses and solver literals belong to the adaptor.
use crate::ast::Expression;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use uniplate::Uniplate;

/// A Boolean operation whose truth is asserted or bound to a named output.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Uniplate)]
#[biplate(to=Expression)]
#[biplate(to=crate::ast::DeclarationPtr)]
pub enum SatEncodingDecision {
    /// Assert the semantic Boolean expression using a Tseitin encoding.
    Assert(Expression),
    /// Define an output as equivalent to a semantic Boolean expression.
    Boolean {
        output: Expression,
        expression: Expression,
    },
}

impl Display for SatEncodingDecision {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Assert(expression) => write!(f, "assert({expression}) using tseitin"),
            Self::Boolean { output, expression } => {
                write!(f, "define({output} <-> {expression}) using tseitin")
            }
        }
    }
}

impl SatEncodingDecision {
    /// Semantic expressions held by this decision.
    pub fn expressions(&self) -> Vec<&Expression> {
        match self {
            Self::Assert(expression) => vec![expression],
            Self::Boolean { output, expression } => vec![output, expression],
        }
    }
}

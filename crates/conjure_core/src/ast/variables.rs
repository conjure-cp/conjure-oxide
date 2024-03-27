use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::ast::domains::{Domain, Range};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionVariable {
    pub domain: Domain,
}

impl DecisionVariable {
    pub fn new(domain: Domain) -> DecisionVariable {
        DecisionVariable { domain }
    }
}

impl Display for DecisionVariable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.domain {
            Domain::BoolDomain => write!(f, "bool"),
            Domain::IntDomain(ranges) => {
                let mut first = true;
                for r in ranges {
                    if first {
                        first = false;
                    } else {
                        write!(f, " or ")?;
                    }
                    match r {
                        Range::Single(i) => write!(f, "{}", i)?,
                        Range::Bounded(i, j) => write!(f, "{}..{}", i, j)?,
                    }
                }
                Ok(())
            }
        }
    }
}

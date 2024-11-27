use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};
use uniplate::derive::Uniplate;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Uniplate, Hash)]
#[uniplate()]

/// A literal value, equivalent to constants in Conjure.
pub enum Literal {
    Int(i32),
    Bool(bool),
}

impl TryFrom<Literal> for i32 {
    type Error = &'static str;

    fn try_from(value: Literal) -> Result<Self, Self::Error> {
        match value {
            Literal::Int(i) => Ok(i),
            _ => Err("Cannot convert non-i32 literal to i32"),
        }
    }
}
impl TryFrom<Literal> for bool {
    type Error = &'static str;

    fn try_from(value: Literal) -> Result<Self, Self::Error> {
        match value {
            Literal::Bool(b) => Ok(b),
            _ => Err("Cannot convert non-bool literal to bool"),
        }
    }
}

impl From<i32> for Literal {
    fn from(i: i32) -> Self {
        Literal::Int(i)
    }
}

impl From<bool> for Literal {
    fn from(b: bool) -> Self {
        Literal::Bool(b)
    }
}

impl Display for Literal {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            Literal::Int(i) => write!(f, "{}", i),
            Literal::Bool(b) => write!(f, "{}", b),
        }
    }
}

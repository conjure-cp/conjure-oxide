use std::fmt::{Display, Formatter};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Constant {
    Int(i32),
    Bool(bool),
}

impl TryFrom<Constant> for i32 {
    type Error = &'static str;

    fn try_from(value: Constant) -> Result<Self, Self::Error> {
        match value {
            Constant::Int(i) => Ok(i),
            _ => Err("Cannot convert non-i32 Constant to i32"),
        }
    }
}
impl TryFrom<Constant> for bool {
    type Error = &'static str;

    fn try_from(value: Constant) -> Result<Self, Self::Error> {
        match value {
            Constant::Bool(b) => Ok(b),
            _ => Err("Cannot convert non-bool Constant to bool"),
        }
    }
}

impl Display for Constant {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            Constant::Int(i) => write!(f, "Int({})", i),
            Constant::Bool(b) => write!(f, "Bool({})", b),
        }
    }
}

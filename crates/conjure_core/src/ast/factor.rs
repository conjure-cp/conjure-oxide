use crate::metadata::Metadata;

use super::{Expression, Literal, Name};
use serde::{Deserialize, Serialize};
use uniplate::derive::Uniplate;

/// A `Factor` is an indivisible expression, such as a literal or a reference.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Uniplate)]
#[uniplate()]
#[biplate(to=Name)]
#[biplate(to=Literal)]
#[biplate(to=Metadata)]
#[biplate(to=Expression)]
pub enum Factor {
    Literal(Literal),
    Reference(Name),
}

impl std::fmt::Display for Factor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Factor::Literal(x) => x.fmt(f),
            Factor::Reference(Name::UserName(n)) => {
                write!(f, "{n}")?;
                Ok(())
            }

            Factor::Reference(Name::MachineName(x)) => {
                write!(f, "${x}")?;
                Ok(())
            }
        }
    }
}

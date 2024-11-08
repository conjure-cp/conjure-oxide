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

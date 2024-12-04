use crate::metadata::Metadata;

use super::{Expression, Literal, Name};
use serde::{Deserialize, Serialize};
use uniplate::derive::Uniplate;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Uniplate)]
#[uniplate()]
#[biplate(to=Name)]
#[biplate(to=Literal)]
#[biplate(to=Metadata)]
#[biplate(to=Expression)]
pub enum Atom {
    Literal(Literal),
    Reference(Name),
}

impl std::fmt::Display for Atom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Atom::Literal(x) => x.fmt(f),
            Atom::Reference(x) => x.fmt(f),
        }
    }
}

impl From<Literal> for Atom {
    fn from(value: Literal) -> Self {
        Atom::Literal(value)
    }
}

impl From<Name> for Atom {
    fn from(value: Name) -> Self {
        Atom::Reference(value)
    }
}

use itertools::Itertools;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Hash, Quine)]
pub enum ReturnType {
    Int,
    Bool,
    Matrix(Box<ReturnType>),
    Set(Box<ReturnType>),
    MSet(Box<ReturnType>),
    Tuple(Vec<ReturnType>),
    Record(Vec<ReturnType>),
    Function(Box<ReturnType>, Box<ReturnType>),

    /// An unknown type
    ///
    /// This can be found inside the types of empty abstract literals.
    ///
    /// To understand why, consider the typing of a set literal.  We construct the type of a set
    /// literal by looking at the type of its items (e.g. {1,2,3} is type `set(int)`, as 1 is an
    /// int). However, if it has no items, we can't do this, so we give it the type `set(unknown)`.
    Unknown,
}

/// Guaranteed to always typecheck
pub trait Typeable {
    fn return_type(&self) -> ReturnType;
}

impl Display for ReturnType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ReturnType::Bool => write!(f, "Bool"),
            ReturnType::Int => write!(f, "Int"),
            ReturnType::Matrix(inner) => write!(f, "Matrix of {inner}"),
            ReturnType::Set(inner) => write!(f, "Set of {inner}"),
            ReturnType::MSet(inner) => write!(f, "Multiset of {inner}"),
            ReturnType::Tuple(types) => {
                let inners = types.iter().map(|t| format!("{}", t)).join(", ");
                write!(f, "({inners})")
            }
            ReturnType::Record(types) => {
                let inners = types.iter().map(|t| format!("{}", t)).join(", ");
                write!(f, "Record {{ {inners} }}")
            }
            ReturnType::Function(ty1, ty2) => {
                write!(f, "Function ({ty1} -> {ty2})")
            }
            ReturnType::Unknown => write!(f, "?"),
        }
    }
}

use crate::ast::Field;
use itertools::Itertools;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Hash, Quine)]
/// Variants use the project-wide type/domain ordering; keep broad matches in the same order.
pub enum ReturnType {
    /// A type which is not known locally, but can be inferred from context.
    ///
    /// Type unification will resolve this to the contextual type.
    Unknown,
    Bool,
    Int,
    Tuple(Vec<ReturnType>),
    Record(Vec<Field<ReturnType>>),
    Variant(Vec<Field<ReturnType>>),
    Matrix(Box<ReturnType>),
    Sequence(Box<ReturnType>),
    Set(Box<ReturnType>),
    MSet(Box<ReturnType>),
    Function(Box<ReturnType>, Box<ReturnType>),
    Relation(Vec<ReturnType>),
    Partition(Box<ReturnType>),
    Permutation(Box<ReturnType>),
}

impl ReturnType {
    /// If this is a collection of elements of the same type (e.g. matrix / set),
    /// get the element type. Otherwise, returns None.
    pub fn elem_type(&self) -> Option<ReturnType> {
        match self {
            ReturnType::Matrix(e)
            | ReturnType::Sequence(e)
            | ReturnType::Set(e)
            | ReturnType::MSet(e) => Some(*e.clone()),
            _ => None,
        }
    }
}

/// Guaranteed to always typecheck
pub trait Typeable {
    fn return_type(&self) -> ReturnType;
}

impl Display for Field<ReturnType> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.value)
    }
}

impl Display for ReturnType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ReturnType::Unknown => write!(f, "?"),
            ReturnType::Bool => write!(f, "bool"),
            ReturnType::Int => write!(f, "int"),
            ReturnType::Tuple(types) => {
                let inners = types.iter().map(|t| format!("{}", t)).join(", ");
                write!(f, "tuple of ({inners})")
            }
            ReturnType::Record(types) => {
                let inners = types.iter().map(|t| format!("{}", t)).join(", ");
                write!(f, "record of {{{inners}}}")
            }
            ReturnType::Variant(types) => {
                let inners = types.iter().map(|t| format!("{}", t)).join(", ");
                write!(f, "variant {{{inners}}}")
            }
            ReturnType::Matrix(inner) => write!(f, "matrix of {inner}"),
            ReturnType::Sequence(inner) => write!(f, "sequence of {inner}"),
            ReturnType::Set(inner) => write!(f, "set of {inner}"),
            ReturnType::MSet(inner) => write!(f, "mset of {inner}"),
            ReturnType::Function(ty1, ty2) => {
                write!(f, "function of ({ty1} --> {ty2})")
            }
            ReturnType::Relation(types) => {
                let inners = types.iter().map(|t| format!("{}", t)).join(", ");
                write!(f, "relation of ({inners})")
            }
            ReturnType::Partition(inner) => write!(f, "partition of {inner}"),
            ReturnType::Permutation(inner) => write!(f, "permutation of {inner}"),
        }
    }
}

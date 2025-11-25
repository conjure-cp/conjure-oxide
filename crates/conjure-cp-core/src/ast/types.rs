use polyquine::Quine;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Hash, Quine)]
pub enum ReturnType {
    Int,
    Bool,
    Matrix(Box<ReturnType>),
    Set(Box<ReturnType>),
    Tuple(Vec<ReturnType>),
    Record(Vec<ReturnType>),

    /// An unknown type
    ///
    /// This can be found inside the types of empty abstract literals.
    ///
    /// To understand why, consider the typing of a set literal.  We construct the type of a set
    /// literal by looking at the type of its items (e.g. {1,2,3} is type `set(int)`, as 1 is an
    /// int). However, if it has no items, we can't do this, so we give it the type `set(unknown)`.
    Unknown,
}

/// Something with a return type
pub trait Typeable {
    fn return_type(&self) -> Option<ReturnType>;
}

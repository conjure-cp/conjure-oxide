use serde::{Deserialize, Serialize};

use super::Range;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum ReturnType {
    Int,
    Bool,
    Set,

    /// A matrix or 1-dimensional slice.
    Matrix {
        /// The indices of each dimension of the matrix.
        dimensions: Vec<Vec<Range<i32>>>,

        /// The type of values in the matrix.
        values: Box<ReturnType>,
    },
}

/// Something with a return type
pub trait Typeable {
    fn return_type(&self) -> Option<ReturnType>;
}

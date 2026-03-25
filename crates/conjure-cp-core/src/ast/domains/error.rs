use crate::ast::ReturnType;
use crate::bug;
use crate::utils::CombinatoricsError;
use std::num::TryFromIntError;
use thiserror::Error;

/// An error thrown by an operation on domains.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum DomainOpError {
    #[error(
        "The operation only supports bounded / finite domains, but was given an unbounded input domain."
    )]
    Unbounded,

    #[error("The operation only supports integer input domains, but got a {0:?} input domain.")]
    NotInteger(ReturnType),

    #[error("The operation was given input domains of the wrong type.")]
    WrongType,

    #[error("The operation failed as the input domain was not ground")]
    NotGround,

    #[error("Result would exceed the bounds of this integer domain")]
    OutOfBounds,

    #[error("Could not enumerate the domain as it is too large")]
    TooLarge,
}

impl From<CombinatoricsError> for DomainOpError {
    fn from(value: CombinatoricsError) -> Self {
        match value {
            CombinatoricsError::Overflow => Self::TooLarge,
            CombinatoricsError::NotDefined(msg) => {
                bug!("Are we passing the right arguments here? ({})", msg)
            }
        }
    }
}

impl From<TryFromIntError> for DomainOpError {
    fn from(value: TryFromIntError) -> Self {
        Self::OutOfBounds
    }
}

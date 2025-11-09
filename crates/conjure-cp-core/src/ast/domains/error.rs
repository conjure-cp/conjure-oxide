use crate::ast::ReturnType;
use crate::bug;
use crate::utils::CombinatoricsError;
use thiserror::Error;

/// An error thrown by an operation on domains.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[allow(clippy::enum_variant_names)] // all variant names start with Input at the moment, but that is ok.
pub enum DomainOpError {
    /// The operation only supports bounded / finite domains, but was given an unbounded input domain.
    #[error(
        "The operation only supports bounded / finite domains, but was given an unbounded input domain."
    )]
    InputUnbounded,

    /// The operation only supports integer input domains, but was given an input domain of a
    /// different type.
    #[error("The operation only supports integer input domains, but got a {0:?} input domain.")]
    InputNotInteger(ReturnType),

    /// The operation was given an input domain of the wrong type.
    #[error("The operation was given input domains of the wrong type.")]
    InputWrongType,

    /// The operation failed as the input domain contained a reference.
    #[error("The operation failed as the input domain contained a reference")]
    InputContainsReference,

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

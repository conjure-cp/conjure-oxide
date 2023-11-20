//! Error types for Minion bindings.

use crate::raw_bindings::*;
use thiserror::Error;

/// A wrapper over all errors thrown by `minion_rs`.
///
/// `Error` allows functions involving Minion to return a single error type. All error types in
/// `minion_rs` are able to be converted into this type using into / from.
#[derive(Debug, Error)]
pub enum MinionError {
    #[error("runtime error: `{0}.to_string()`")]
    RuntimeError(#[from] RuntimeError),
    #[error("not implemented: {0}")]
    NotImplemented(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error), // source and Display delegate to anyhow::Error
}

/// RuntimeErrors are thrown by Minion during execution.
///
/// These represent internal Minion C++ exceptions translated into Rust.
///
/// Invalid usage of this library should throw an error before Minion is even run. Therefore, these
/// should be quite rare. Consider creating an issue on
/// [Github](https://github.com/conjure-cp/conjure-oxide) if these occur regularly!
#[derive(Debug, Error)]
pub enum RuntimeError {
    // These closely follow the ReturnCodes found in Minion's libwrapper.cpp.
    /// The model given to Minion is invalid.
    #[error("the given instance is invalid")]
    InvalidInstance,

    /// An unknown error has occurred.
    #[error("an unknown error has occurred while running minion")]
    UnknownError,
}

// Minion's ReturnCodes are passed over FFI as ints.
// Convert them to their appropriate error.
impl From<u32> for RuntimeError {
    fn from(return_code: u32) -> Self {
        match return_code {
            #[allow(non_upper_case_globals)]
            ReturnCodes_INVALID_INSTANCE => RuntimeError::InvalidInstance,
            _ => RuntimeError::UnknownError,
        }
    }
}

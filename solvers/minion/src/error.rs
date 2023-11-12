//! Error types for Minion bindings.

use crate::raw_bindings::*;
use thiserror::Error;

/// RuntimeErrors are thrown by Minion during execution.
#[derive(Debug, Error)]
pub enum RuntimeError {
    // These closely follow the ReturnCodes found in Minion's libwrapper.cpp.
    #[error("the given instance is invalid")]
    InvalidInstance,

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

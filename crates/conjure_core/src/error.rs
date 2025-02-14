//! Top-level error types for Conjure-Oxide.

use serde_json::Error as JsonError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("JSON error: {0}")]
    JSON(#[from] JsonError),

    #[error("Error parsing model: {0}")]
    Parse(String),

    #[error("{0} is not yet implemented.")]
    NotImplemented(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

// Macro to add an error with the line number and function name
#[macro_export]
macro_rules! throw_error {
    ($msg:expr) => {{
        let error_msg = format!(
            " {} | File: {} | Function: {} | Line: {}",
            $msg,
            file!(),
            module_path!(),
            line!()
        );
        Err(Error::Parse(error_msg))
    }};
}

// Macro to add an error with the line number and function name
#[macro_export]
macro_rules! error {
    ($msg:expr) => {{
        let error_msg = format!(
            " {} | File: {} | Function: {} | Line: {}",
            $msg,
            file!(),
            module_path!(),
            line!()
        );
        Error::Parse(error_msg)
    }};
}

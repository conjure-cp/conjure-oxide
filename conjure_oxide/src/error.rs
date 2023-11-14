use serde_json::Error as JsonError;
use thiserror::Error;
use std::{fmt::Display};

#[derive(Debug, Error)]
pub enum Error {
    #[error("serde_json error: {0}")]
    Json(JsonError),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Error: {0}")]
    Generic(String),
}

impl From<JsonError> for Error {
    fn from(e: JsonError) -> Self {
        Error::Json(e)
    }
}

impl From<&str> for Error {
    fn from(e: &str) -> Self {
        Error::Generic(e.to_owned())
    }
}

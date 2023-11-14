use serde_json::Error as JsonError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("serde_json error: {0}")]
    Json(#[from] JsonError),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Error: {0}")]
    Generic(String),
}

impl From<&str> for Error {
    fn from(e: &str) -> Self {
        Error::Generic(e.to_owned())
    }
}

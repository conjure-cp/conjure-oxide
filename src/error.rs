use serde_json::Error as JsonError;
use std::fmt::Display;

pub enum Error {
    Json(JsonError),
    ParseError(String),
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

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self {
            Error::Json(e) => write!(f, "serde_json error: {}", e),
            Error::ParseError(e) => write!(f, "Parse error: {}", e),
            Error::Generic(e) => write!(f, "Error: {}", e),
        }
    }
}

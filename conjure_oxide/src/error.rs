use serde_json::Error as JsonError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] JsonError),
    #[error("Error constructing model: {0}")]
    ModelConstructError(String),
}

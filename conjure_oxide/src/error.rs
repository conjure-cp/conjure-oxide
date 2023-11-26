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

    #[error("The preconditions for the given rule `{0}.name` were not satisfied.")]
    RuleNotApplicable(Rule),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub use conjure_cp_core::error::Error as ConjureParseError;
use conjure_cp_core::error::Error;
use serde_json::Error as JsonError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum EssenceParseError {
    #[error("Could not parse Essence AST: {0}")]
    TreeSitterError(String),
    #[error("Error running `conjure pretty`: {0}")]
    ConjurePrettyError(String),
    #[error("Essence syntax error: {msg}{}",
        match range {
            Some(range) => format!(" at {}-{}", range.start_point, range.end_point),
            None => "".to_string(),
        }
    )]
    SyntaxError {
        msg: String,
        range: Option<tree_sitter::Range>,
    },
    #[error("JSON Error: {0}")]
    JsonError(#[from] JsonError),
    #[error("Error: {0} is not yet implemented.")]
    NotImplemented(String),
    #[error("Error: {0}")]
    Other(Error),
}

impl EssenceParseError {
    pub fn syntax_error(msg: String, range: Option<tree_sitter::Range>) -> Self {
        EssenceParseError::SyntaxError { msg, range }
    }
}

impl From<ConjureParseError> for EssenceParseError {
    fn from(value: ConjureParseError) -> Self {
        match value {
            Error::Parse(msg) => EssenceParseError::syntax_error(msg, None),
            Error::NotImplemented(msg) => EssenceParseError::NotImplemented(msg),
            Error::Json(err) => EssenceParseError::JsonError(err),
            Error::Other(err) => EssenceParseError::Other(err.into()),
        }
    }
}

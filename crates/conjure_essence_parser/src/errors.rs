pub use conjure_cp_core::error::Error as ConjureParseError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum EssenceParseError {
    #[error("Could not parse Essence AST: {0}")]
    TreeSitterError(String),
    #[error("Error running conjure pretty: {0}")]
    ConjurePrettyError(String),
    #[error("Error running conjure solve: {0}")]
    ConjureSolveError(String),
    #[error("Error parsing essence file: {0}")]
    ParseError(ConjureParseError),
    #[error("Error parsing Conjure solutions file: {0}")]
    ConjureSolutionsError(String),
    #[error("No solutions file for {0}")]
    ConjureNoSolutionsFile(String),
}

impl From<ConjureParseError> for EssenceParseError {
    fn from(e: ConjureParseError) -> Self {
        EssenceParseError::ParseError(e)
    }
}

impl From<&str> for EssenceParseError {
    fn from(e: &str) -> Self {
        EssenceParseError::ParseError(ConjureParseError::Parse(e.to_string()))
    }
}

impl From<String> for EssenceParseError {
    fn from(e: String) -> Self {
        EssenceParseError::ParseError(ConjureParseError::Parse(e))
    }
}

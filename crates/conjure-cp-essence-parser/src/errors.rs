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
        file_name: Option<String>,
        source_code: Option<String>,
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
        EssenceParseError::SyntaxError {
            msg,
            range,
            file_name: None,
            source_code: None,
        }
    }

    /// Format the error in a pretty way with source context
    pub fn pretty_format(&self) -> String {
        match self {
            EssenceParseError::SyntaxError {
                msg,
                range,
                file_name,
                source_code,
            } => {
                // If we have all the info, format nicely
                if let (Some(range), Some(file_name), Some(source_code)) =
                    (range, file_name, source_code)
                {
                    let line_num = range.start_point.row + 1; // tree-sitter uses 0-indexed rows
                    let col_num = range.start_point.column + 1; // tree-sitter uses 0-indexed columns

                    // Get the specific line from source code
                    let lines: Vec<&str> = source_code.lines().collect();
                    let line_content = lines.get(range.start_point.row).unwrap_or(&"");

                    // Build the pointer line (spaces + ^)
                    let pointer = " ".repeat(range.start_point.column) + "^";

                    format!(
                        "{}:{}:{}:\n  |\n{} | {}\n  | {}\n{}",
                        file_name, line_num, col_num, line_num, line_content, pointer, msg
                    )
                } else {
                    // Fall back to simple format
                    format!("{}", self)
                }
            }
            _ => {
                // For other error types, use the Display impl
                format!("{}", self)
            }
        }
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

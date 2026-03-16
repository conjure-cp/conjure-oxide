pub use conjure_cp_core::error::Error as ConjureParseError;
use conjure_cp_core::error::Error;
use serde_json::Error as JsonError;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum FatalParseError {
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
    ParseError {
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

impl FatalParseError {
    pub fn syntax_error(msg: String, range: Option<tree_sitter::Range>) -> Self {
        FatalParseError::ParseError { msg, range }
    }
}

impl From<ConjureParseError> for FatalParseError {
    fn from(value: ConjureParseError) -> Self {
        match value {
            Error::Parse(msg) => FatalParseError::syntax_error(msg, None),
            Error::NotImplemented(msg) => FatalParseError::NotImplemented(msg),
            Error::Json(err) => FatalParseError::JsonError(err),
            Error::Other(err) => FatalParseError::Other(err.into()),
        }
    }
}

#[derive(Debug)]
pub struct RecoverableParseError {
    pub msg: String,
    pub range: Option<tree_sitter::Range>,
    pub file_name: Option<String>,
    pub source_code: Option<String>,
}

impl RecoverableParseError {
    pub fn new(msg: String, range: Option<tree_sitter::Range>) -> Self {
        Self {
            msg,
            range,
            file_name: None,
            source_code: None,
        }
    }

    pub fn enrich(mut self, file_name: Option<String>, source_code: Option<String>) -> Self {
        self.file_name = file_name;
        self.source_code = source_code;
        self
    }
}

impl std::fmt::Display for RecoverableParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // If we have all the info, format nicely with source context
        if let (Some(range), Some(file_name), Some(source_code)) =
            (&self.range, &self.file_name, &self.source_code)
        {
            let line_num = range.start_point.row + 1; // tree-sitter uses 0-indexed rows
            let col_num = range.start_point.column + 1; // tree-sitter uses 0-indexed columns

            // Get the specific line from source code
            let lines: Vec<&str> = source_code.lines().collect();
            let line_content = lines.get(range.start_point.row).unwrap_or(&"");

            // Build the pointer line (spaces + ^)
            let pointer = " ".repeat(range.start_point.column) + "^";

            write!(
                f,
                "{}:{}:{}:\n  |\n{} | {}\n  | {}\n{}",
                file_name, line_num, col_num, line_num, line_content, pointer, self.msg
            )
        } else {
            // Fall back to simple format without context
            write!(f, "Essence syntax error: {}", self.msg)?;
            if let Some(range) = &self.range {
                write!(f, " at {}-{}", range.start_point, range.end_point)?;
            }
            Ok(())
        }
    }
}

/// Collection of parse errors
#[derive(Debug)]
pub enum ParseErrorCollection {
    /// A single fatal error that stops parsing entirely
    Fatal(FatalParseError),
    /// Multiple recoverable errors accumulated during parsing
    Multiple { errors: Vec<RecoverableParseError> },
}

impl ParseErrorCollection {
    /// Create a fatal error collection from a single fatal error
    pub fn fatal(error: FatalParseError) -> Self {
        ParseErrorCollection::Fatal(error)
    }

    /// Create a multiple error collection from recoverable errors
    /// This enriches all errors with file_name and source_code
    pub fn multiple(
        errors: Vec<RecoverableParseError>,
        source_code: Option<String>,
        file_name: Option<String>,
    ) -> Self {
        let enriched_errors = errors
            .into_iter()
            .map(|err| err.enrich(file_name.clone(), source_code.clone()))
            .collect();
        ParseErrorCollection::Multiple {
            errors: enriched_errors,
        }
    }
}

impl std::fmt::Display for ParseErrorCollection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseErrorCollection::Fatal(error) => write!(f, "{}", error),
            ParseErrorCollection::Multiple { errors } => {
                // Create indices sorted by line and column
                let mut indices: Vec<usize> = (0..errors.len()).collect();
                indices.sort_by(|&a, &b| {
                    match (&errors[a], &errors[b]) {
                        (
                            RecoverableParseError {
                                range: Some(r1), ..
                            },
                            RecoverableParseError {
                                range: Some(r2), ..
                            },
                        ) => {
                            // Compare by row first, then by column
                            match r1.start_point.row.cmp(&r2.start_point.row) {
                                std::cmp::Ordering::Equal => {
                                    r1.start_point.column.cmp(&r2.start_point.column)
                                }
                                other => other,
                            }
                        }
                        // Errors without ranges go last
                        (RecoverableParseError { range: Some(_), .. }, _) => {
                            std::cmp::Ordering::Less
                        }
                        (_, RecoverableParseError { range: Some(_), .. }) => {
                            std::cmp::Ordering::Greater
                        }
                        _ => std::cmp::Ordering::Equal,
                    }
                });

                // Print out each error using Display
                for (i, &idx) in indices.iter().enumerate() {
                    if i > 0 {
                        write!(f, "\n\n")?;
                    }
                    write!(f, "{}", errors[idx])?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ParseErrorCollection {}

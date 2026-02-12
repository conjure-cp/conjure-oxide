pub use conjure_cp_core::error::Error as ConjureParseError;
use conjure_cp_core::error::Error;
use serde_json::Error as JsonError;

#[derive(Debug)]
pub enum EssenceParseError {
    TreeSitterError(String),
    ConjurePrettyError(String),
    ParseError {
        msg: String,
        range: Option<tree_sitter::Range>,
        file_name: Option<String>,
        source_code: Option<String>,
    },
    JsonError(JsonError),
    NotImplemented(String),
    Other(Error),
}

impl EssenceParseError {
    pub fn syntax_error(msg: String, range: Option<tree_sitter::Range>) -> Self {
        EssenceParseError::ParseError {
            msg,
            range,
            file_name: None,
            source_code: None,
        }
    }
}

impl std::fmt::Display for EssenceParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EssenceParseError::TreeSitterError(msg) => {
                write!(f, "Could not parse Essence AST: {}", msg)
            }
            EssenceParseError::ConjurePrettyError(msg) => {
                write!(f, "Error running `conjure pretty`: {}", msg)
            }
            EssenceParseError::ParseError {
                msg,
                range,
                file_name,
                source_code,
            } => {
                // If we have all the info, format nicely with source context
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

                    write!(
                        f,
                        "{}:{}:{}:\n  |\n{} | {}\n  | {}\n{}",
                        file_name, line_num, col_num, line_num, line_content, pointer, msg
                    )
                } else {
                    // Fall back to simple format without context
                    write!(f, "Essence syntax error: {}", msg)?;
                    if let Some(range) = range {
                        write!(f, " at {}-{}", range.start_point, range.end_point)?;
                    }
                    Ok(())
                }
            }
            EssenceParseError::JsonError(err) => write!(f, "JSON Error: {}", err),
            EssenceParseError::NotImplemented(msg) => {
                write!(f, "Error: {} is not yet implemented.", msg)
            }
            EssenceParseError::Other(err) => write!(f, "Error: {}", err),
        }
    }
}

impl std::error::Error for EssenceParseError {}

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

/// Collection of parse errors
#[derive(Debug)]
pub struct ParseErrorCollection {
    pub errors: Vec<EssenceParseError>,
}

impl ParseErrorCollection {
    pub fn new(errors: Vec<EssenceParseError>) -> Self {
        Self { errors }
    }
}

impl std::fmt::Display for ParseErrorCollection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Create indices sorted by line and column
        let mut indices: Vec<usize> = (0..self.errors.len()).collect();
        indices.sort_by(|&a, &b| {
            match (&self.errors[a], &self.errors[b]) {
                (
                    EssenceParseError::ParseError {
                        range: Some(r1), ..
                    },
                    EssenceParseError::ParseError {
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
                (EssenceParseError::ParseError { range: Some(_), .. }, _) => {
                    std::cmp::Ordering::Less
                }
                (_, EssenceParseError::ParseError { range: Some(_), .. }) => {
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
            write!(f, "{}", self.errors[idx])?;
        }
        Ok(())
    }
}

impl std::error::Error for ParseErrorCollection {}

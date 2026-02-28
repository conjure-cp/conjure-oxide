// Basic syntactic error detection helpers for the LSP API.

use crate::diagnostics::diagnostics_api::{Diagnostic, Position, Range, Severity};
use crate::errors::RecoverableParseError;
use crate::parse_essence_with_context;
use conjure_cp_core::context::Context;
use std::sync::{Arc, RwLock};

pub fn detect_errors(source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut errors = vec![];
    let context = Arc::new(RwLock::new(Context::default()));

    match parse_essence_with_context(source, context, &mut errors) {
        Ok(_model) => {
            // Convert all recoverable errors to diagnostics
            for error in errors {
                diagnostics.push(error_to_diagnostic(&error));
            }
        }
        Err(_fatal) => {
            // Fatal error means something went wrong internally (e.g., tree-sitter parser failure)
            // We can't provide meaningful diagnostics in this case, so just return empty
            // TODO: Figure out how LSP should handle fatal errors from the parser.
        }
    }

    diagnostics
}

pub fn error_to_diagnostic(err: &RecoverableParseError) -> Diagnostic {
    let (start, end) = range_to_position(&err.range);
    Diagnostic {
        range: Range { start, end },
        severity: Severity::Error,
        source: "semantic error detection",
        message: err.msg.clone(),
    }
}

fn range_to_position(range: &Option<tree_sitter::Range>) -> (Position, Position) {
    match range {
        Some(r) => (
            Position {
                line: r.start_point.row as u32,
                character: r.start_point.column as u32,
            },
            Position {
                line: r.end_point.row as u32,
                character: r.end_point.column as u32,
            },
        ),
        None => (
            Position {
                line: 0,
                character: 0,
            },
            Position {
                line: 0,
                character: 0,
            },
        ),
    }
}

/// Helper function for tests to compare the actual diagnostic with the expected one.
pub fn check_diagnostic(
    diag: &Diagnostic,
    line_start: u32,
    char_start: u32,
    line_end: u32,
    char_end: u32,
    msg: &str,
) {
    // Checking range
    assert_eq!(diag.range.start.line, line_start);
    assert_eq!(diag.range.start.character, char_start);
    assert_eq!(diag.range.end.line, line_end);
    assert_eq!(diag.range.end.character, char_end);

    // Check the message
    assert_eq!(diag.message, msg);
}

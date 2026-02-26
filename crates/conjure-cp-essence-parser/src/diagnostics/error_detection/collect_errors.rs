// Basic syntactic error detection helpers for the LSP API.

use crate::diagnostics::diagnostics_api::{Diagnostic, Position, Range, Severity};
use crate::errors::RecoverableParseError;
use crate::{FatalParseError, parse_essence_with_context};
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
        Err(fatal) => {
            // For now, convert fatal errors to diagnostics too
            // Since many errors that should be recoverable are still using FatalParseError::ParseError
            diagnostics.push(fatal_error_to_diagnostic(&fatal));
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

pub fn fatal_error_to_diagnostic(err: &FatalParseError) -> Diagnostic {
    match err {
        crate::FatalParseError::ParseError { msg, range } => {
            let (start, end) = range_to_position(range);
            Diagnostic {
                range: Range { start, end },
                severity: Severity::Error,
                source: "semantic error detection",
                message: msg.clone(),
            }
        }
        _ => Diagnostic {
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 1,
                },
            },
            severity: Severity::Error,
            source: "semantic error detection",
            message: format!("{}", err),
        },
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

/// Helper function
pub fn print_diagnostics(diags: &[Diagnostic]) {
    for (i, diag) in diags.iter().enumerate() {
        println!(
            "Diagnostic {}:\n  Range: ({}:{}) - ({}:{})\n  Severity: {:?}\n  Message: {}\n  Source: {}\n",
            i + 1,
            diag.range.start.line,
            diag.range.start.character,
            diag.range.end.line,
            diag.range.end.character,
            diag.severity,
            diag.message,
            diag.source
        );
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

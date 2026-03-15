// Basic syntactic error detection helpers for the LSP API.

use crate::diagnostics::diagnostics_api::{Diagnostic, Position, Range, Severity};
use crate::errors::RecoverableParseError;
use crate::parse_essence_with_context_and_map;
use crate::parser::keyword_checks::keyword_as_identifier;
use conjure_cp_core::context::Context;
use std::sync::{Arc, RwLock};
use tree_sitter::Tree;

pub fn detect_errors(source: &str, cst: &Tree) -> Vec<Diagnostic> {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut errors: Vec<RecoverableParseError> = vec![];
    let context = Arc::new(RwLock::new(Context::default()));

    keyword_as_identifier(cst.root_node(), source, &mut errors);
    match parse_essence_with_context_and_map(source, context, &mut errors, Some(cst)) {
        Ok(_) => {
            diagnostics.extend(errors.into_iter().map(|e| error_to_diagnostic(&e)));
        }
        Err(_) => {}
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

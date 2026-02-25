// Basic syntactic error detection helpers for the LSP API.

use crate::diagnostics::diagnostics_api::{Diagnostic, Position, Range, Severity};
use crate::errors::RecoverableParseError;
use crate::{FatalParseError, parse_essence_with_context};
use conjure_cp_core::context::Context;
use std::sync::{Arc, RwLock};

/// Detects very simple semantic issues in source and returns a vector
/// of Diagnostics.
pub fn detect_semantic_errors(source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let context = Arc::new(RwLock::new(Context::default()));
    let mut errors = vec![];

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
        message: format!("Semantic Error: {}", err.msg),
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

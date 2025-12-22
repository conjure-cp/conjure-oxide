// Basic syntactic error detection helpers for the LSP API.

use crate::diagnostics::diagnostics_api::{Diagnostic, Position, Range, Severity};
use crate::parse_essence_with_context;
use conjure_cp_core::context::Context;
use std::sync::{Arc, RwLock};

/// Detects very simple semantic issues in source and returns a vector
/// of Diagnostics.
pub fn detect_semantic_errors(source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let context = Arc::new(RwLock::new(Context::default()));

    match parse_essence_with_context(source, context) {
        Ok(_model) => {
            // no errors, all good
        }
        Err(err) => {
            diagnostics.push(error_to_diagnostic(&err));
        }
    }

    diagnostics
}

pub fn error_to_diagnostic(err: &crate::errors::EssenceParseError) -> Diagnostic {
    match err {
        crate::EssenceParseError::SyntaxError { msg, range } => {
            let (start, end) = range_to_position(range);
            Diagnostic {
                range: Range { start, end },
                severity: Severity::Error,
                source: "semantic error detection",
                message: format!("Semantic Error: {}", msg),
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

// Basic syntactic error detection helpers for the LSP API.

use crate::lsp_api::{Diagnostic, Position, Range, severity};

/// Detects very simple syntactic issues in source and returns a vector
/// of Diagnostics.
///
/// just counting open and close parentheses for now
/// (to be refactored into a separate function later)
pub fn detect_syntactic_errors(source: &str) -> Vec<Diagnostic> {
    // panic!("to be implemented in sub PR");

    let mut diagnostics = Vec::new();

    // use CST to detect unmatched parentheses
    // by calling the tree-sitter parser on the source code
    // instead of just counting characters
    let open_count = source.matches('(').count();
    let close_count = source.matches(')').count();

    if open_count > close_count {
        let message = format!(
            "Unmatched opening parenthesis: {}",
            open_count - close_count
        );
    } else if close_count > open_count {
        let message = format!(
            "Unmatched closing parenthesis: {}",
            close_count - open_count
        );
    }

    // push to diagnostics
    diagnostics.push(Diagnostic {
        // range is just start of file, has to be more specific.
        range: Range {
            start: Position { line: 0, character: 0 },
            end: Position { line: 0, character: 1 },
        },
        severity: severity::Error,
        message,
        source: "syntactic-error-detector",
    });

    diagnostic
}

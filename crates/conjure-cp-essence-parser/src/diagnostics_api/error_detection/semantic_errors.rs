// Basic syntactic error detection helpers for the LSP API.

use crate::diagnostics_api::diagnostics_api::{Diagnostic, Position, Range, severity};

/// Detects very simple semantic issues in source and returns a vector
/// of Diagnostics.
pub fn detect_semantic_errors(source: &str) -> Vec<Diagnostic> {
    panic!("to be implemented in sub PR");
}

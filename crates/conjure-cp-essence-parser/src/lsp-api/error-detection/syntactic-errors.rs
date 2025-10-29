// Basic syntactic error detection helpers for the LSP API.

use crate::lsp_api::{Diagnostic, Position, Range, severity};

/// Detects very simple syntactic issues in source and returns a vector
/// of Diagnostics.
pub fn detect_syntactic_errors(source: &str) -> Vec<Diagnostic> {
    panic!("to be implemented in sub PR");
}

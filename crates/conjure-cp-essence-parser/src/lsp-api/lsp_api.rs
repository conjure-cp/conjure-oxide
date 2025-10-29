use serde::{Deserialize, Serialize}
use tree_sitter::{Node, Point}

use crate::parser::util::{get_tree, named_children}
use crate::parser::{find::parse_find_statement, letting::parse_letting_statement}

// structs for lsp stuff

// position / range
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

// the actual values can be chnaged later, if needed
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum severity {
    Error = 1,
    Warn = 2,
    Info = 3,
    Hint = 4,
}

// the actual diagnostic struct
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub range: Range,
    pub severity: Severity,
    pub message: String,
    pub source: &'static str,
}

// document symbol struct is used to denote a single token / node
// this will be used for syntax highlighting
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SymbolKind {
    integer = 0,
    decimal = 1,
    function = 2,
    letting = 3,
    find = 4,
} // to be extended

// each type of token / symbol in the essence grammar will be
// assigned an integer, which would be mapped to a colour
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: SymbolKind,
    pub range: Range,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<DocumentSymbol>>,
}

// getting the actual diagnostic
pub fn get_diagnostics(source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

        // semantic error detection from error-detection/semantic-errors.rs
        diagnostics.extend(detect_semantic_errors(source)); // not implemented yet

        // syntactic error detection from error-detection/syntactic-errors.rs
        diagnostics.extend(detect_syntactic_errors(source)); // not implemented yet

    }
    diagnostics
}

// get document symbols for semantic highlighting

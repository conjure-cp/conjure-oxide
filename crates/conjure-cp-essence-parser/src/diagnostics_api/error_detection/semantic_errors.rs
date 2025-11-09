// Basic syntactic error detection helpers for the LSP API.

use crate::diagnostics_api::diagnostics_api::{Diagnostic, Position, Range, severity};
use crate::parser::util::{get_tree};
use tree_sitter::{Node};


/// Detects very simple semantic issues in source and returns a vector
/// of Diagnostics.
pub fn detect_semantic_errors(source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let (tree, source) = match get_tree(source) {
        Some(tree) => tree,
        // essence cannot be parsed
        None => {
            // get the position of last character to get the range of the entire source code
            let last_line = source.lines().count().saturating_sub(1);
            let last_char = source.lines().last().map(|l| l.len()).unwrap_or(0);
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position { line: 0, character: 0 },
                    end: Position {line: last_line as u32, character: last_char as u32},
                },
                severity: severity::Error,
                message: "Failed to read the source code".to_string(),
                source: "Tree-Sitter-Parse-Error",
            });
        return diagnostics
        }

    };

    let root_node = tree.root_node();
    // call semantic error detection functions
    keyword_as_identifier(root_node, &source, &mut diagnostics);

    diagnostics
}

const KEYWORDS: [&str; 20] = [
    "forall", "exists", "such", "that", "letting", "find", "minimise", "maximise",
    "subject", "to", "where", "and", "or", "not", "if", "then", "else", "in",
    "sum", "product"
];


// keyword as identifier error and push to diagnostics
fn keyword_as_identifier(root: Node, src: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "variable" {
            if let Ok(text) = node.utf8_text(src.as_bytes()) {
                let ident = text.trim();
                if KEYWORDS.contains(&ident) {
                    let start_point = node.start_position();
                    let end_point = node.end_position();
                    diagnostics.push(Diagnostic {
                        range: Range {
                            start: Position { line: start_point.row as u32, character: start_point.column as u32 },
                            end: Position { line: end_point.row as u32, character: end_point.column as u32 },
                        },
                        severity: severity::Error,
                        message: format!("Keyword '{}' used as an identifier", ident),
                        source: "semantic-error-detector",
                    });
                }
            }
        }
    }
}

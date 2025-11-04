// Basic syntactic error detection helpers for the LSP API.

use crate::diagnostics_api::diagnostics_api::{Diagnostic, Position, Range, severity};
use crate::parser::util::{get_tree, named_children};
use tree_sitter::{Node};


/// Detects very simple syntactic issues in source and returns a vector
/// of Diagnostics.
///
/// just counting open and close parentheses for now
/// (to be refactored into a separate function later)
pub fn detect_syntactic_errors(source: &str) -> Vec<Diagnostic> {


    let mut diagnostics = Vec::new();

    // Walk the tree with DFS 
    // If error -> classify the error and push 
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

    if root_node.has_error() {
        // Walk the tree using preorder DFS to find error nodes and classify them 
        let mut stack = vec![root_node];
        while let Some(node) = stack.pop() {
            if (node.is_error() || node.is_missing()) &&
                (!node.parent().map_or(false, |p| p.is_error() || p.is_missing())) {
                // passing the top-level error node
                diagnostics.push(classify_syntax_error(node));
            }
            // DFS traversal
            for i in (0..node.child_count()).rev() {
                if let Some(child) = node.child(i) {
                    stack.push(child);
                }
            }
        }
    }

    diagnostics
}
fn deepest_error_node(node: Node) -> Node {
    let mut current = node;
    loop {
        // Find the first child that is error or missing
        let mut found = false;
        for i in 0..current.child_count() {
            if let Some(child) = current.child(i) {
                if child.is_error() || child.is_missing() {
                    current = child;
                    found = true;
                    break;
                }
            }
        }
        if !found {
            break;
        }
    }
    current
}

fn classify_syntax_error(node: Node) -> Diagnostic {
    let deepest = deepest_error_node(node);

    if deepest.is_missing() {
        classify_missing_token(deepest)
    }
    else {
        classify_general_syntax_error(deepest)
    }
}

// Missing token 
fn classify_missing_token(node: Node) -> Diagnostic {
    let start = node.start_position();
    let end = node.end_position();

    Diagnostic {
        range: Range {
            start: Position { line: start.row as u32, character: start.column as u32 },
            end: Position { line: end.row as u32, character: end.column as u32 },
        },
        severity: severity::Error,
        message: format!("Missing token: '{}'", node.kind()),
        source: "syntactic-error-detector",
    }
}

fn classify_general_syntax_error(node: Node) -> Diagnostic {
    let start = node.start_position();
    let end = node.end_position();
    // print!( "Missing token".to_string());
    Diagnostic {
        range: Range {
            start: Position { line: start.row as u32, character: start.column as u32 },
            end: Position { line: end.row as u32, character: end.column as u32 },
        },
        severity: severity::Error,
        message: "Missing token".to_string(),
        source: "syntactic-error-detector",
    }

}





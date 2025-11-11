
use crate::diagnostics::diagnostics_api::{Diagnostic, Position, Range, Severity};
use crate::parser::util::get_tree;
use tree_sitter::{Node};

/// Traverses all nodes in the parse tree and prints all error and missing nodes with their kind and range.
/// Prints each error or missing node's kind and range to stdout.
pub fn print_all_error_nodes(source: &str) {
    if let Some((tree, _)) = get_tree(source) {
        let root_node = tree.root_node();
        let mut stack = vec![root_node];
        while let Some(node) = stack.pop() {
            if node.is_error() || node.is_missing() {
                println!(
                    "[all errors] Error node: '{}' [{}:{}-{}:{}] (missing: {})",
                    node.kind(),
                    node.start_position().row, node.start_position().column,
                    node.end_position().row, node.end_position().column,
                    node.is_missing()
                );
            }
            for i in (0..node.child_count()).rev() {
                if let Some(child) = node.child(i) {
                    stack.push(child);
                }
            }
        }
    } else {
        println!("[all errors] Could not parse source.");
    }
}


/// Detects syntactic issues in the essence source text and returns a vector of Diagnostics.
///
/// This function traverses the parse tree, looking for missing or error nodes, and generates
/// diagnostics for each. It uses a DFS and skips children of error/missing nodes
/// to avoid duplicate diagnostics. If the source cannot be parsed, a diagnostic is returned for that.
///
/// # Arguments
/// * `source` - The source code to analyze.
///
/// # Returns
/// * `Vec<Diagnostic>` - A vector of diagnostics describing syntactic issues found in the source.
pub fn detect_syntactic_errors(source: &str) -> Vec<Diagnostic> {

    let mut diagnostics = Vec::new();

    let (tree, _) = match get_tree(source) {
        Some(tree) => tree,
        None => {
            let last_line = source.lines().count().saturating_sub(1);
            let last_char = source.lines().last().map(|l| l.len()).unwrap_or(0);
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position { line: 0, character: 0 },
                    end: Position { line: last_line as u32, character: last_char as u32 },
                },
                severity: Severity::Error,
                message: "Failed to read the source code".to_string(),
                source: "Tree-Sitter-Parse-Error",
            });
            return diagnostics;
        }
    };

    let root_node = tree.root_node();

    // Stack-based DFS, skip children of error/missing nodes (only report top-level errors)
    let mut stack = vec![root_node];
    while let Some(node) = stack.pop() {

        // Detect all the missing nodes before since tree-sitter sometimes is not able to correctly identify a missing node.
        // Use zero-width range check and move on to avoid duplicate diagnistics
        if node.start_position() == node.end_position() {
            diagnostics.push(classify_missing_token(node));
            continue;
        }

        if (node.is_error()) || node.is_missing() &&
            (!node.parent().is_some_and(|p| p.is_error() || p.is_missing())) {

            diagnostics.push(classify_syntax_error(node));
            // stops traversing children of error/missing nodes (will do that when classifying)
            continue;
        }

        // Otherwise, traverse children
        for i in (0..node.child_count()).rev() {
            if let Some(child) = node.child(i) {
                stack.push(child);
            }
        }
    }

    diagnostics
}


/// Classifies a syntax error node and returns a diagnostic for it.
fn classify_syntax_error(node: Node) -> Diagnostic {

    if node.start_position() == node.end_position() {
        classify_missing_token(node)
    } else {
        classify_general_syntax_error(node)
    }
}


/// Classifies a missing token node and generates a diagnostic with a context-aware message.
fn classify_missing_token(node: Node) -> Diagnostic {
    let start = node.start_position();
    let end = node.end_position();

    let message = if let Some(parent) = node.parent() {
        match parent.kind() {
            "letting_statement" => "Missing 'expression or domain'".to_string(),
            "and_expr" => "Missing right operand in 'and' expression".to_string(),
            "comparison_expr" => "Missing right operand in 'comparison' expression".to_string(),
            _ => format!("Missing '{}'", node.kind()),
        }
    } else {
        format!("Missing '{}'", node.kind())
    };

    Diagnostic {
        range: Range {
            start: Position { line: start.row as u32, character: start.column as u32 },
            end: Position { line: end.row as u32, character: end.column as u32 },
        },
        severity: Severity::Error,
        message,
        source: "syntactic-error-detector",
    }
}


/// Classifies a general syntax error that cannot be classified with other functions.
fn classify_general_syntax_error(node: Node) -> Diagnostic {
    let start = node.start_position();
    let end = node.end_position();

    // Try to provide more context in the message
    let message = if let Some(parent) = node.parent() {
        format!(
            "Syntax error in '{}': unexpected or invalid '{}'.",
            parent.kind(),
            node.kind()
        )
    } else {
        format!("Syntax error: unexpected or invalid '{}'.", node.kind())
    };

    Diagnostic {
        range: Range {
            start: Position { line: start.row as u32, character: start.column as u32 },
            end: Position { line: end.row as u32, character: end.column as u32 },
        },
        severity: Severity::Error,
        message,
        source: "syntactic-error-detector",
    }
}


// fn classify_unexpected_token() -> Diagnostic {

// }

// if ERROR at top level children, uknown command

/// Helper function for tests to compare the actual diagnostic with the expected one.
pub fn check_diagnostic(

    diag: &Diagnostic,
    line_start: u32,
    char_start: u32,
    line_end: u32,
    char_end: u32,
    msg: &str) {

    // Checking range
    assert_eq!(diag.range.start.line, line_start);
    assert_eq!(diag.range.start.character, char_start);
    assert_eq!(diag.range.end.line, line_end);
    assert_eq!(diag.range.end.character, char_end);

    // Check the message
    assert_eq!(diag.message, msg);

}

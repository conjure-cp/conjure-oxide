use crate::diagnostics::diagnostics_api::{Diagnostic, Position, Range, Severity};
use crate::parser::util::get_tree;
use tree_sitter::Node;

/// Helper function to see all the error nodes tree-sitter generated.
/// Prints each error or missing node's.
pub fn print_all_error_nodes(source: &str) {
    if let Some((tree, _)) = get_tree(source) {
        let root_node = tree.root_node();
        println!("{}", root_node.to_sexp());
        let mut stack = vec![root_node];
        while let Some(node) = stack.pop() {
            if node.is_error() || node.is_missing() {
                println!(
                    "[all errors] Error node: '{}' [{}:{}-{}:{}] (children: {})",
                    node.kind(),
                    node.start_position().row,
                    node.start_position().column,
                    node.end_position().row,
                    node.end_position().column,
                    node.child_count()
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

/// Helper function
pub fn print_diagnostics(diags: &Vec<Diagnostic>) {
    for (i, diag) in diags.iter().enumerate() {
        println!(
            "Diagnostic {}:\n  Range: ({}:{}) - ({}:{})\n  Severity: {:?}\n  Message: {}\n  Source: {}\n",
            i + 1,
            diag.range.start.line,
            diag.range.start.character,
            diag.range.end.line,
            diag.range.end.character,
            diag.severity,
            diag.message,
            diag.source
        );
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
        _none => {
            let last_line = source.lines().count().saturating_sub(1);
            let last_char = source.lines().last().map(|l| l.len()).unwrap_or(0);
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: last_line as u32,
                        character: last_char as u32,
                    },
                },
                severity: Severity::Error,
                message: "Failed to read the source code".to_string(),
                source: "Tree-Sitter-Parse-Error",
            });
            return diagnostics;
        }
    };

    let root_node = tree.root_node();
    let mut cursor = root_node.walk();

    // let mut descend = true;
    loop {
        let node = cursor.node();

        // Detect all the missing nodes before since tree-sitter sometimes is not able to correctly identify a missing node.
        // Use zero-width range check and move on to avoid duplicate diagnostics
        let descend = if node.start_position() == node.end_position() {
            diagnostics.push(classify_missing_token(node));
            false
        } else if (node.is_error() || node.is_missing())
            && (!node
                .parent()
                .map_or(false, |p| p.is_error() || p.is_missing()))
        {
            diagnostics.push(classify_syntax_error(node, source));
            false
        } else {
            true
        };

        // TreeCursor traversal: preorder DFS, skip children of error/missing nodes
        if descend && cursor.goto_first_child() {
            continue;
        }
        if cursor.goto_next_sibling() {
            continue;
        }
        // Go up until we can go to a next sibling, or break if at root
        while cursor.goto_parent() {
            if cursor.goto_next_sibling() {
                break;
            }
        }
        // If we're back at the root and can't go further, break
        if cursor.node() == root_node {
            break;
        }
    }

    diagnostics
}

/// Classifies a syntax error node and returns a diagnostic for it.
fn classify_syntax_error(node: Node, source: &str) -> Diagnostic {
    let (start, end) = (node.start_position(), node.end_position());

    if node.is_missing() {
        return classify_missing_token(node);
    }

    let message = if is_unexpected_token(node) {
        // If no children (exept the token itself) - unexpected token
        classify_unexpected_token_error(node, source)
    } else {
        classify_general_syntax_error(node)
    };
    Diagnostic {
        range: Range {
            start: Position {
                line: start.row as u32,
                character: start.column as u32,
            },
            end: Position {
                line: end.row as u32,
                character: end.column as u32,
            },
        },
        severity: Severity::Error,
        message,
        source: "syntactic-error-detector",
    }
}

/// When an unexpected sybmol is part of the grammar - token, CST produces one ERROR node
/// If not part of the grammar - two nested ERROR nodes.
/// For misplaces integers - one ERROR node with no children, for everything else, one child node
/// !is_named() is used to detect string literals
fn is_unexpected_token(node: Node) -> bool {
    return node.child_count() == 0
        || node.child_count() == 1
            && (!node.child(0).unwrap().is_named() || (node.child(0).unwrap().is_error()));
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
            start: Position {
                line: start.row as u32,
                character: start.column as u32,
            },
            end: Position {
                line: end.row as u32,
                character: end.column as u32,
            },
        },
        severity: Severity::Error,
        message,
        source: "syntactic-error-detector",
    }
}

fn classify_unexpected_token_error(node: Node, source_code: &str) -> String {
    // println!(
    //     "Error node: '{}' [{}:{}-{}:{}]",
    //     node.kind(),
    //     node.start_position().row,
    //     node.start_position().column,
    //     node.end_position().row,
    //     node.end_position().column,
    // );
    let message = if let Some(parent) = node.parent() {
        let src_token = &source_code[node.start_byte()..node.end_byte()];

        // Unexpected token at the end of a statement
        if parent.kind() == "program" {
            // Save cursor position
            if let Some(prev_sib) = node.prev_sibling().and_then(|n| n.prev_sibling()) {
                format!(
                    "Unexpected '{}' at the end of '{}'",
                    src_token,
                    prev_sib.kind()
                )
            } else {
                format!("Unexpected '{}' ", src_token)
            }
        } else {
            format!("Unexpected '{}' inside '{}'", src_token, parent.kind())
        }
    // Error at root node (program)
    } else {
        format!("Unexpected '{}", source_code)
    };

    message
}

/// Classifies a general syntax error that cannot be classified with other functions.
fn classify_general_syntax_error(node: Node) -> String {
    if let Some(parent) = node.parent() {
        format!(
            "Syntax error in '{}': unexpected or invalid '{}'.",
            parent.kind(),
            node.kind()
        )
    } else {
        format!("Syntax error: unexpected or invalid '{}'.", node.kind())
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

#[test]
fn error_at_start() {
    let source = "; find x: int(1..3)";
    let diagnostics = detect_syntactic_errors(source);
    assert!(!diagnostics.is_empty(), "Expected at least one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(diag, 0, 0, 0, 19, "Failed to read the source code'");
}

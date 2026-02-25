use crate::diagnostics::diagnostics_api::{Diagnostic, Position, Range, Severity};
use crate::parser::traversal::WalkDFS;
use crate::parser::util::get_tree;
use capitalize::Capitalize;
use std::collections::HashSet;
use tree_sitter::Node;

/// Helper function
pub fn print_diagnostics(diags: &[Diagnostic]) {
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

/// Returns true if the node's start or end column is out of range for its line in the source.
fn error_node_out_of_range(node: &tree_sitter::Node, source: &str) -> bool {
    let lines: Vec<&str> = source.lines().collect();
    let start = node.start_position();
    let end = node.end_position();

    let start_line_len = lines.get(start.row).map_or(0, |l| l.len());
    let end_line_len = lines.get(end.row).map_or(0, |l| l.len());

    (start.column > start_line_len) || (end.column > end_line_len)
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
    let mut malformed_lines_reported = HashSet::new();

    let (tree, _) = match get_tree(source) {
        Some(tree) => tree,
        None => {
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
    let retract: &dyn Fn(&tree_sitter::Node) -> bool = &|node: &tree_sitter::Node| {
        node.is_missing() || node.is_error() || node.start_position() == node.end_position()
    };

    for node in WalkDFS::with_retract(&root_node, &retract) {
        if node.start_position() == node.end_position() {
            diagnostics.push(classify_missing_token(node));
            continue;
        }
        if node.is_error() {
            let line = node.start_position().row;
            // If this line has already been reported as malformed, skip all error nodes on this line
            if malformed_lines_reported.contains(&line) {
                continue;
            }
            if is_malformed_line_error(&node, source) {
                malformed_lines_reported.insert(line);

                let last_char = source.lines().nth(line).map_or(0, |l| l.len());
                diagnostics.push(generate_a_syntax_err_diagnostic(
                    line as u32,
                    0,
                    line as u32,
                    last_char as u32,
                    &format!(
                        "Malformed line {}: '{}'",
                        line + 1,
                        source.lines().nth(line).unwrap_or("")
                    ),
                ));
                continue;
            } else {
                diagnostics.push(classify_unexpected_token_error(node, source));
            }
            continue;
        }
    }

    diagnostics
}

/// Classifies a missing token node and generates a diagnostic with a context-aware message.
fn classify_missing_token(node: Node) -> Diagnostic {
    let start = node.start_position();
    let end = node.end_position();

    let message = if let Some(parent) = node.parent() {
        match parent.kind() {
            "letting_statement" => "Missing Expression or Domain".to_string(),
            _ => format!("Missing {}", user_friendly_token_name(node.kind(), false)),
        }
    } else {
        format!("Missing {}", user_friendly_token_name(node.kind(), false))
    };

    generate_a_syntax_err_diagnostic(
        start.row as u32,
        start.column as u32,
        end.row as u32,
        end.column as u32,
        &message,
    )
}

/// Classifies an unexpected token error node and generates a diagnostic.
fn classify_unexpected_token_error(node: Node, source_code: &str) -> Diagnostic {
    let message = if let Some(parent) = node.parent() {
        let start_byte = node.start_byte().min(source_code.len());
        let end_byte = node.end_byte().min(source_code.len());
        let src_token = &source_code[start_byte..end_byte];

        if parent.kind() == "program"
        // ERROR node is the direct child of the root node
        {
            // A case where the unexpected token is at the end of a valid statement
            format!("Unexpected {}", src_token)
            // }
        } else {
            // Unexpected token inside a construct
            format!(
                "Unexpected {} inside {}",
                src_token,
                user_friendly_token_name(parent.kind(), true)
            )
        }
    } else {
        // Should never happen since an ERROR node would always have a parent.
        "Unexpected token".to_string()
    };

    generate_a_syntax_err_diagnostic(
        node.start_position().row as u32,
        node.start_position().column as u32,
        node.end_position().row as u32,
        node.end_position().column as u32,
        &message,
    )
}

/// Determines if an error node represents a malformed line error.
fn is_malformed_line_error(node: &tree_sitter::Node, source: &str) -> bool {
    if node.start_position().column == 0 || error_node_out_of_range(node, source) {
        return true;
    }
    let parent = node.parent();
    let grandparent = parent.and_then(|n| n.parent());
    let root = grandparent.and_then(|n| n.parent());

    if let (Some(parent), Some(grandparent), Some(root)) = (parent, grandparent, root) {
        parent.kind() == "set_operation_bool"
            && grandparent.kind() == "bool_expr"
            && root.kind() == "program"
    } else {
        false
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

/// Coverts a token name into a more user-friendly format for error messages.
/// Removes underscores, replaces certain keywords with more natural language, and adds appropriate articles.
fn user_friendly_token_name(token: &str, article: bool) -> String {
    let capitalized = if token.contains("atom") {
        "Expression".to_string()
    } else if token == "COLON" {
        ":".to_string()
    } else {
        let friendly_name = token
            .replace("literal", "")
            .replace("int", "Integer")
            .replace("expr", "Expression")
            .replace('_', " ");
        friendly_name
            .split_whitespace()
            .map(|word| word.capitalize())
            .collect::<Vec<_>>()
            .join(" ")
    };

    if !article {
        return capitalized;
    }
    let first_char = capitalized.chars().next().unwrap();
    let article = match first_char.to_ascii_lowercase() {
        'a' | 'e' | 'i' | 'o' | 'u' => "an",
        _ => "a",
    };
    format!("{} {}", article, capitalized)
}

fn generate_a_syntax_err_diagnostic(
    line_start: u32,
    char_start: u32,
    line_end: u32,
    char_end: u32,
    msg: &str,
) -> Diagnostic {
    Diagnostic {
        range: Range {
            start: Position {
                line: line_start,
                character: char_start,
            },
            end: Position {
                line: line_end,
                character: char_end,
            },
        },
        severity: Severity::Error,
        message: msg.to_string(),
        source: "syntactic-error-detector",
    }
}

#[test]
fn error_at_start() {
    let source = "; find x: int(1..3)";
    let diagnostics = detect_syntactic_errors(source);
    assert!(!diagnostics.is_empty(), "Expected at least one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(diag, 0, 0, 0, 19, "Failed to read the source code");
}

#[test]
fn user_friendly_token_name_article() {
    assert_eq!(
        user_friendly_token_name("int_domain", false),
        "Integer Domain"
    );
    assert_eq!(
        user_friendly_token_name("int_domain", true),
        "an Integer Domain"
    );
    // assert_eq!(user_friendly_token_name("atom", true), "an Expression");
    assert_eq!(user_friendly_token_name("COLON", false), ":");
}
#[test]
fn malformed_line() {
    let source = " a,a,b: int(1..3)";
    let (tree, _) = get_tree(source).expect("Should parse");
    let root_node = tree.root_node();

    let error_node = WalkDFS::with_retract(&root_node, &|_node| false)
        .find(|node| node.is_error())
        .expect("Should find an error node");

    assert!(is_malformed_line_error(&error_node, source));
}

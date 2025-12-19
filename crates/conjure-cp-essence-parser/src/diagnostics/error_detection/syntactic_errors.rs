use crate::diagnostics::diagnostics_api::{Diagnostic, Position, Range, Severity};
use crate::parser::traversal::WalkDFS;
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
            if node.is_error() || node.is_missing() || node.has_error() {
                println!(
                    "Error: '{}' [{}:{}-{}:{}] (children: {}) parent: {}",
                    node.kind(),
                    node.start_position().row,
                    node.start_position().column,
                    node.end_position().row,
                    node.end_position().column,
                    node.child_count(),
                    node.parent()
                        .map_or("None".to_string(), |p| p.kind().to_string())
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

    let start_line_len = lines.get(start.row as usize).map_or(0, |l| l.len());
    let end_line_len = lines.get(end.row as usize).map_or(0, |l| l.len());

    (start.column as usize > start_line_len) || (end.column as usize > end_line_len)
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
    // Retract (do not descend) if the node is missing, error, or their parent is missing/error
    let retract = |node: &tree_sitter::Node| {
        node.is_missing() || node.is_error() || node.start_position() == node.end_position()
    };

    for node in WalkDFS::with_retract(&root_node, &retract) {
        // Tree-sitter sometimes fails to insert a MISSING node, do a range check to be sure
        if node.start_position() == node.end_position() {
            diagnostics.push(classify_missing_token(node));
            continue;
        }
        // Only classify error nodes whose parent is not error/missing
        if (node.is_error())
            && !node
                .parent()
                .is_some_and(|p| p.is_error() || p.is_missing())
        {
            diagnostics.push(classify_syntax_error(node, source));
            continue;
        }
    }

    diagnostics
}

/// Classifies a syntax error node and returns a diagnostic for it.
fn classify_syntax_error(node: Node, source: &str) -> Diagnostic {
    if node.is_missing() {
        return classify_missing_token(node);
    } else if node.is_error() {
        classify_unexpected_token_error(node, source)
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
        print!("Hello");
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
fn classify_unexpected_token_error(node: Node, source_code: &str) -> Diagnostic {
    let (message, whole_line, line_index) = if let Some(parent) = node.parent() {
        let start_byte = node.start_byte().min(source_code.len());
        let end_byte = node.end_byte().min(source_code.len());
        let src_token = &source_code[start_byte..end_byte];

        // Malformed entire lines
        // Tree-sitter cannot apply any grammar rule to a line

        // ERROR node is the direct child of the root node
        if parent.kind() == "program" {
            let li = node.start_position().row as usize;
            let line_text = source_code.lines().nth(li).unwrap_or("");

            // happens when the malformed line is the first
            // Tree-sitter places the error node out of range, needs separate handling
            if error_node_out_of_range(&node, source_code) {
                (
                    format!("Malformed line {}: '{}'", li + 1, line_text),
                    true,
                    li,
                )
            } else if node.start_position().column == 0 {
                (
                    format!("Malformed line {}: '{}'", li + 1, line_text),
                    true,
                    li,
                )
            // Unexpected tokens

            // Tree-sitter classified a line but found unexpected token at the end of it
            } else if let Some(prev_sib) = node.prev_sibling().and_then(|n| n.prev_sibling()) {
                (
                    format!(
                        "Unexpected '{}' at the end of '{}'",
                        src_token,
                        prev_sib.kind()
                    ),
                    false,
                    li,
                )
            } else {
                (format!("Unexpected '{}'", src_token), false, li)
            }
        // Unexpected tokens inside constructs
        } else {
            (
                format!("Unexpected '{}' inside '{}'", src_token, parent.kind()),
                false,
                0,
            )
        }
    } else {
        (format!("Unexpected '{}'", source_code), false, 0)
    };

    // compute range once based on whole_line flag or node positions
    let (start, end) = if whole_line {
        let li = line_index;
        let line_text = source_code.lines().nth(li).unwrap_or("");
        (
            Position {
                line: li as u32,
                character: 0,
            },
            Position {
                line: li as u32,
                character: line_text.len() as u32,
            },
        )
    } else {
        (
            Position {
                line: node.start_position().row as u32,
                character: node.start_position().column as u32,
            },
            Position {
                line: node.end_position().row as u32,
                character: node.end_position().column as u32,
            },
        )
    };

    Diagnostic {
        range: Range { start, end },
        severity: Severity::Error,
        message,
        source: "syntactic-error-detector",
    }
}

/// Classifies a general syntax error that cannot be classified with other functions.
fn classify_general_syntax_error(node: Node) -> Diagnostic {
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
            start: Position {
                line: node.start_position().row as u32,
                character: node.start_position().column as u32,
            },
            end: Position {
                line: node.end_position().row as u32,
                character: node.end_position().column as u32,
            },
        },
        severity: Severity::Error,
        message,
        source: "syntactic-error-detector",
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
    check_diagnostic(diag, 0, 0, 0, 19, "Failed to read the source code");
}

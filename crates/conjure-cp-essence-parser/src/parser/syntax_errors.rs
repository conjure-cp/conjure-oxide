use crate::errors::RecoverableParseError;
use crate::parser::traversal::WalkDFS;
use capitalize::Capitalize;
use std::collections::HashSet;
use tree_sitter::Node;

pub fn classify_error_node(node: Node, source: &str) -> RecoverableParseError {
    let line = node.start_position().row;
    // If this line has already been reported as malformed, skip all error nodes on this line

    if is_malformed_line_error(&node, source) {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();

        let last_char = source.lines().nth(line).map_or(0, |l| l.len());
        RecoverableParseError::new(
            format!(
                "Malformed line {}: '{}'",
                line + 1,
                source.lines().nth(line).unwrap_or("")
            ),
            Some(tree_sitter::Range {
                start_byte,
                end_byte,
                start_point: tree_sitter::Point {
                    row: line,
                    column: 0,
                },
                end_point: tree_sitter::Point {
                    row: line,
                    column: last_char,
                },
            }),
        )
    } else {
        classify_unexpected_token_error(node, source)
    }
}

pub fn detect_syntactic_errors(
    source: &str,
    tree: &tree_sitter::Tree,
) -> Vec<RecoverableParseError> {
    let mut errors: Vec<RecoverableParseError> = Vec::new();
    let mut malformed_lines_reported = HashSet::new();

    let root_node = tree.root_node();
    let retract: &dyn Fn(&tree_sitter::Node) -> bool = &|node: &tree_sitter::Node| {
        node.is_missing() || node.is_error() || node.start_position() == node.end_position()
    };

    for node in WalkDFS::with_retract(&root_node, &retract) {
        if node.start_position() == node.end_position() {
            errors.push(classify_missing_token(node));
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
                let start_byte = node.start_byte();
                let end_byte = node.end_byte();

                let last_char = source.lines().nth(line).map_or(0, |l| l.len());
                errors.push(RecoverableParseError::new(
                    format!(
                        "Malformed line {}: '{}'",
                        line + 1,
                        source.lines().nth(line).unwrap_or("")
                    ),
                    Some(tree_sitter::Range {
                        start_byte,
                        end_byte,
                        start_point: tree_sitter::Point {
                            row: line,
                            column: 0,
                        },
                        end_point: tree_sitter::Point {
                            row: line,
                            column: last_char,
                        },
                    }),
                ));
                continue;
            } else {
                errors.push(classify_unexpected_token_error(node, source));
            }
            continue;
        }
    }

    errors
}

/// Classifies a missing token node and generates a diagnostic with a context-aware message.
fn classify_missing_token(node: Node) -> RecoverableParseError {
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

    RecoverableParseError::new(
        message,
        Some(tree_sitter::Range {
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_point: start,
            end_point: end,
        }),
    )
}

/// Classifies an unexpected token error node and generates a diagnostic.
fn classify_unexpected_token_error(node: Node, source_code: &str) -> RecoverableParseError {
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

    RecoverableParseError::new(
        message,
        Some(tree_sitter::Range {
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_point: node.start_position(),
            end_point: node.end_position(),
        }),
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

/// Returns true if the node's start or end column is out of range for its line in the source.
fn error_node_out_of_range(node: &tree_sitter::Node, source: &str) -> bool {
    let lines: Vec<&str> = source.lines().collect();
    let start = node.start_position();
    let end = node.end_position();

    let start_line_len = lines.get(start.row).map_or(0, |l| l.len());
    let end_line_len = lines.get(end.row).map_or(0, |l| l.len());

    (start.column > start_line_len) || (end.column > end_line_len)
}

#[cfg(test)]
mod test {

    use super::{detect_syntactic_errors, is_malformed_line_error, user_friendly_token_name};
    use crate::errors::RecoverableParseError;
    use crate::{parser::traversal::WalkDFS, util::get_tree};

    /// Helper function for tests to compare the actual error with the expected one.
    fn assert_essence_parse_error_eq(a: &RecoverableParseError, b: &RecoverableParseError) {
        assert_eq!(a.msg, b.msg, "error messages differ");
        assert_eq!(a.range, b.range, "error ranges differ");
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
    fn missing_domain() {
        let source = "find x:";
        let (tree, _) = get_tree(source).expect("Should parse");
        let errors = detect_syntactic_errors(source, &tree);
        assert_eq!(errors.len(), 1, "Expected exactly one diagnostic");

        let error = &errors[0];

        assert_essence_parse_error_eq(
            error,
            &RecoverableParseError::new(
                "Missing Domain".to_string(),
                Some(tree_sitter::Range {
                    start_byte: 7,
                    end_byte: 7,
                    start_point: tree_sitter::Point { row: 0, column: 7 },
                    end_point: tree_sitter::Point { row: 0, column: 7 },
                }),
            ),
        );
    }
}

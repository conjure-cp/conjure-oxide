use crate::errors::RecoverableParseError;
use crate::parser::traversal::WalkDFS;
use capitalize::Capitalize;
use std::collections::HashSet;
use tree_sitter::Node;

/// Returns the absolute byte offset of the start of `row` in `source`.
fn line_start_byte(source: &[u8], row: usize) -> usize {
    let mut current_row = 0usize;
    let mut line_start = 0usize;
    for (idx, b) in source.iter().enumerate() {
        if current_row == row {
            break;
        }
        if *b == b'\n' {
            current_row += 1;
            line_start = idx + 1;
        }
    }
    line_start
}

/// This is a reporting-layer fix: even though comments are treated as "extras" by the grammar,
/// tree-sitter `ERROR` node spans can overlap those bytes during recovery. We clamp to the end of
/// the non-comment prefix (with trailing whitespace trimmed) so diagnostics don't include comment
/// contents.
fn clamp_range_before_line_comment(range: &mut tree_sitter::Range, source: &str) {
    let Some(line) = source.lines().nth(range.start_point.row) else {
        return;
    };
    let Some(dollar_idx) = line.find('$') else {
        return;
    };

    let prefix = &line[..dollar_idx];
    let clamped_col = prefix.trim_end().len();

    if range.start_point.column > clamped_col {
        range.start_point.column = clamped_col;
    }
    if range.end_point.row == range.start_point.row && range.end_point.column > clamped_col {
        range.end_point.column = clamped_col;
    }
    if range.end_point.row > range.start_point.row {
        range.end_point.row = range.start_point.row;
        range.end_point.column = clamped_col;
    }

    let line_start = line_start_byte(source.as_bytes(), range.start_point.row);
    range.start_byte = line_start + range.start_point.column;
    range.end_byte = line_start + range.end_point.column;
}

pub fn detect_syntactic_errors(
    source: &str,
    tree: &tree_sitter::Tree,
    errors: &mut Vec<RecoverableParseError>,
) {
    let mut malformed_lines_reported = HashSet::new();

    let root_node = tree.root_node();
    let retract: &dyn Fn(&tree_sitter::Node) -> bool = &|node: &tree_sitter::Node| {
        node.is_missing() || node.is_error() || node.start_position() == node.end_position()
    };

    for node in WalkDFS::with_retract(&root_node, &retract) {
        if node.start_position() == node.end_position() {
            errors.push(classify_missing_token(node, source));
            continue;
        }
        if node.is_error() {
            let line = node.start_position().row;
            // If this line has already been reported as malformed, skip all error nodes on this line
            if malformed_lines_reported.contains(&line) {
                continue;
            }
            // Ignore error nodes that start inside a single-line comment.
            if let Some(line_str) = source.lines().nth(line)
                && let Some(dollar_idx) = line_str.find('$')
                && node.start_position().column >= dollar_idx
            {
                continue;
            }

            if is_malformed_line_error(&node, source) {
                malformed_lines_reported.insert(line);
                let start_byte = node.start_byte();
                let end_byte = node.end_byte();

                let last_char = source.lines().nth(line).map_or(0, |l| l.len());
                errors.push(RecoverableParseError::new(
                    generate_malformed_line_message(line, source),
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
}

/// Classifies a missing token node and generates a diagnostic with a context-aware message.
fn classify_missing_token(node: Node, source: &str) -> RecoverableParseError {
    let mut range = tree_sitter::Range {
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        start_point: node.start_position(),
        end_point: node.end_position(),
    };
    clamp_range_before_line_comment(&mut range, source);

    let message = if let Some(parent) = node.parent() {
        match parent.kind() {
            "letting_statement" => "Missing Expression or Domain".to_string(),
            _ => format!("Missing {}", user_friendly_token_name(node.kind(), false)),
        }
    } else {
        format!("Missing {}", user_friendly_token_name(node.kind(), false))
    };

    RecoverableParseError::new(message, Some(range))
}

/// Classifies an unexpected token error node and generates a diagnostic.
fn classify_unexpected_token_error(node: Node, source_code: &str) -> RecoverableParseError {
    let mut range = tree_sitter::Range {
        start_byte: node.start_byte().min(source_code.len()),
        end_byte: node.end_byte().min(source_code.len()),
        start_point: node.start_position(),
        end_point: node.end_position(),
    };
    clamp_range_before_line_comment(&mut range, source_code);

    let message = if let Some(parent) = node.parent() {
        // Extract the unexpected token text, handling out-of-range indices safely.
        // NOTE: tree-sitter byte offsets can land inside UTF-8 codepoints; decoding lossily avoids panics.
        let src_token: std::borrow::Cow<'_, str> = source_code
            .as_bytes()
            .get(range.start_byte..range.end_byte)
            .map(String::from_utf8_lossy)
            .unwrap_or_else(|| std::borrow::Cow::Borrowed("<unknown>"));
        let src_token = src_token.trim_end();

        if parent.kind() == "program" {
            format!("Unexpected {}", src_token)
        } else {
            format!(
                "Unexpected {} inside {}",
                src_token,
                user_friendly_token_name(parent.kind(), true)
            )
        }
    } else {
        "Unexpected token".to_string()
    };

    RecoverableParseError::new(message, Some(range))
}

/// Determines if an error node represents a malformed line error.
pub fn is_malformed_line_error(node: &tree_sitter::Node, source: &str) -> bool {
    if node.start_position().column == 0 || error_node_out_of_range(node, source) {
        return true;
    }
    let parent = node.parent();
    let grandparent = parent.and_then(|n| n.parent());
    let root = grandparent.and_then(|n| n.parent());

    if let (Some(parent), Some(grandparent), Some(root)) = (parent, grandparent, root) {
        parent.kind() == "set_comparison"
            && grandparent.kind() == "comparison_expr"
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

// Generates an informative error message for malformed lines
fn generate_malformed_line_message(line: usize, source: &str) -> String {
    let got = source.lines().nth(line).unwrap_or("").trim();
    let got = got.split('$').next().unwrap_or("").trim_end();
    let got = got.replace('"', "\\\"");
    let mut words = got.split_whitespace();
    let first = words.next().unwrap_or("").to_ascii_lowercase();
    let second = words.next().unwrap_or("").to_ascii_lowercase();

    let expected = match first.as_str() {
        "find" => "a find declaration statement",
        "letting" => "a letting declaration statement",
        "given" => "a given declaration statement",
        "where" => "an instantiation condition",
        "minimising" | "maximising" => "an objective statement",
        "such" => {
            // Check for invalid constraint statement
            if second == "that" {
                "a constraint statement"
            } else {
                "a valid top-level statement"
            }
        }

        _ => {
            // Default case for unrecognized starting tokens
            "a valid top-level statement"
        }
    };
    format!("Expected {}, but got '{}'", expected, got)
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

    use super::{
        clamp_range_before_line_comment, detect_syntactic_errors, is_malformed_line_error,
        line_start_byte, user_friendly_token_name,
    };
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
    fn malformed_find_message() {
        let source = "find >=lex,b,c: int(1..3)";
        let message = super::generate_malformed_line_message(0, source);
        assert_eq!(
            message,
            "Expected a find declaration statement, but got 'find >=lex,b,c: int(1..3)'"
        );
    }

    #[test]
    fn malformed_top_level_message() {
        let source = "a >=lex,b,c: int(1..3)";
        let message = super::generate_malformed_line_message(0, source);
        assert_eq!(
            message,
            "Expected a valid top-level statement, but got 'a >=lex,b,c: int(1..3)'"
        );
    }

    #[test]
    fn malformed_objective_message() {
        let source = "maximising %x";
        let message = super::generate_malformed_line_message(0, source);
        assert_eq!(
            message,
            "Expected an objective statement, but got 'maximising %x'"
        );
    }

    #[test]
    fn malformed_letting_message() {
        let source = "letting % A be 3";
        let message = super::generate_malformed_line_message(0, source);
        assert_eq!(
            message,
            "Expected a letting declaration statement, but got 'letting % A be 3'"
        );
    }

    #[test]
    fn malformed_constraint_message() {
        let source = "such that % A > 3";
        let message = super::generate_malformed_line_message(0, source);
        assert_eq!(
            message,
            "Expected a constraint statement, but got 'such that % A > 3'"
        );
    }

    #[test]
    fn malformed_top_level_message_2() {
        let source = "such % A > 3";
        let message = super::generate_malformed_line_message(0, source);
        assert_eq!(
            message,
            "Expected a valid top-level statement, but got 'such % A > 3'"
        );
    }

    #[test]
    fn malformed_given_message() {
        let source = "given 1..3)";
        let message = super::generate_malformed_line_message(0, source);
        assert_eq!(
            message,
            "Expected a given declaration statement, but got 'given 1..3)'"
        );
    }

    #[test]
    fn malformed_where_message() {
        let source = "where x>6";
        let message = super::generate_malformed_line_message(0, source);
        assert_eq!(
            message,
            "Expected an instantiation condition, but got 'where x>6'"
        );
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
        let mut errors = vec![];
        detect_syntactic_errors(source, &tree, &mut errors);
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

    #[test]
    fn line_start_byte_returns_correct_offsets() {
        let source = "a\nbc\ndef";
        let bytes = source.as_bytes();
        assert_eq!(line_start_byte(bytes, 0), 0);
        assert_eq!(line_start_byte(bytes, 1), 2);
        assert_eq!(line_start_byte(bytes, 2), 5);
    }

    #[test]
    fn clamp_range_before_line_comment_clamps_end_to_before_dollar() {
        let source = "find x: int(1..3 $comment";
        let mut range = tree_sitter::Range {
            start_byte: 0,
            end_byte: source.len(),
            start_point: tree_sitter::Point { row: 0, column: 0 },
            end_point: tree_sitter::Point {
                row: 0,
                column: source.len(),
            },
        };

        clamp_range_before_line_comment(&mut range, source);

        // "find x: int(1..3" ends at byte/column 16; the `$comment` suffix must be excluded.
        assert_eq!(range.end_point.row, 0);
        assert_eq!(range.end_point.column, 16);
        assert_eq!(range.end_byte, 16);
    }
}

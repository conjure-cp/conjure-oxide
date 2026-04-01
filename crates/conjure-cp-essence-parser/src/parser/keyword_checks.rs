use crate::errors::RecoverableParseError;
use crate::parser::syntax_errors::is_malformed_line_error;

const KEYWORDS: [&str; 21] = [
    "forall", "exists", "such", "that", "letting", "find", "minimise", "maximise", "subject", "to",
    "where", "and", "or", "not", "if", "then", "else", "in", "sum", "product", "bool",
];

pub fn keyword_as_identifier(
    root: tree_sitter::Node,
    source: &str,
    errors: &mut Vec<RecoverableParseError>,
) {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.is_error() && is_malformed_line_error(&node, source) {
            return;
        }
        if (node.kind() == "variable" || node.kind() == "identifier" || node.kind() == "parameter")
            && let Ok(text) = node.utf8_text(source.as_bytes())
        {
            let ident = text.trim();
            if KEYWORDS.contains(&ident) {
                let start_point = node.start_position();
                let end_point = node.end_position();
                errors.push(RecoverableParseError::new(
                    format!("Keyword '{ident}' used as identifier"),
                    Some(tree_sitter::Range {
                        start_byte: node.start_byte(),
                        end_byte: node.end_byte(),
                        start_point,
                        end_point,
                    }),
                ));
            }
        }

        for i in 0..node.child_count() {
            if let Some(child) = u32::try_from(i).ok().and_then(|i| node.child(i)) {
                stack.push(child);
            }
        }
    }
}

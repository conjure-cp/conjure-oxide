/// Get the i-th named child of a node, or return a syntax error with a message if it doesn't exist.
#[macro_export]
macro_rules! named_child {
    ($node:ident) => {
        named_child!($node, 0, "Missing sub-expression")
    };
    ($node:ident, $i:literal) => {
        named_child!($node, $i, format!("Missing sub-expression #{}", $i + 1))
    };
    ($node:ident, $i:literal, $msg:expr) => {
        $node
            .named_child($i)
            .ok_or(EssenceParseError::syntax_error(
                format!("{} in expression of kind '{}'", $msg, $node.kind()),
                Some($node.range()),
            ))?
    };
}

/// Get the i-th child of a node, or return a syntax error with a message if it doesn't exist.
#[macro_export]
macro_rules! child {
    ($node:ident) => {
        child!($node, 0, "Missing sub-expression")
    };
    ($node:ident, $i:literal) => {
        child!($node, $i, format!("Missing sub-expression #{}", $i + 1))
    };
    ($node:ident, $i:literal, $msg:expr) => {
        $node.child($i).ok_or(EssenceParseError::syntax_error(
            format!("{} in expression of kind '{}'", $msg, $node.kind()),
            Some($node.range()),
        ))?
    };
}

/// Get the named field of a node, or return a syntax error with a message if it doesn't exist.
#[macro_export]
macro_rules! field {
    ($node:ident, $name:expr) => {
        $node
            .child_by_field_name($name)
            .ok_or(EssenceParseError::syntax_error(
                format!(
                    "Missing field '{}' in expression of kind '{}'",
                    $name,
                    $node.kind()
                ),
                Some($node.range()),
            ))?
    };
}

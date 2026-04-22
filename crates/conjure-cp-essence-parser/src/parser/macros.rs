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
            .ok_or(FatalParseError::internal_error(
                format!("{} in expression of kind '{}'", $msg, $node.kind()),
                Some($node.range()),
            ))?
    };
    // recoverable version
    (recover, $ctx:expr, $node:expr) => {{
        match $node.named_child(0) {
            Some(child) => Some(child),
            None => {
                $ctx.record_error($crate::errors::RecoverableParseError::new(
                    format!(
                        "Missing sub-expression in expression of kind '{}'",
                        $node.kind()
                    ),
                    Some($node.range()),
                ));
                None
            }
        }
    }};
    (recover, $ctx:expr, $node:expr, $i:literal) => {{
        match $node.named_child($i) {
            Some(child) => Some(child),
            None => {
                $ctx.record_error($crate::errors::RecoverableParseError::new(
                    format!(
                        "Missing sub-expression #{} in expression of kind '{}'",
                        $i + 1,
                        $node.kind()
                    ),
                    Some($node.range()),
                ));
                None
            }
        }
    }};
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
        $node.child($i).ok_or(FatalParseError::internal_error(
            format!("{} in expression of kind '{}'", $msg, $node.kind()),
            Some($node.range()),
        ))?
    };
    // recoverable version
    (recover, $ctx:expr, $node:expr) => {{
        match $node.child(0) {
            Some(child) => Some(child),
            None => {
                $ctx.record_error($crate::errors::RecoverableParseError::new(
                    format!(
                        "Missing sub-expression in expression of kind '{}'",
                        $node.kind()
                    ),
                    Some($node.range()),
                ));
                None
            }
        }
    }};
    (recover, $ctx:expr, $node:expr, $i:literal) => {{
        match $node.child($i) {
            Some(child) => Some(child),
            None => {
                $ctx.record_error($crate::errors::RecoverableParseError::new(
                    format!(
                        "Missing sub-expression #{} in expression of kind '{}'",
                        $i + 1,
                        $node.kind()
                    ),
                    Some($node.range()),
                ));
                None
            }
        }
    }};
}

/// Get the named field of a node, or return a syntax error with a message if it doesn't exist.
#[macro_export]
macro_rules! field {
    ($node:ident, $name:expr) => {
        $node
            .child_by_field_name($name)
            .ok_or(FatalParseError::internal_error(
                format!(
                    "Missing field '{}' in expression of kind '{}'",
                    $name,
                    $node.kind()
                ),
                Some($node.range()),
            ))?
    };
    // recoverable version
    (recover, $ctx:expr, $node:expr, $name:expr) => {{
        match $node.child_by_field_name($name) {
            Some(child) => Some(child),
            None => {
                $ctx.record_error($crate::errors::RecoverableParseError::new(
                    format!(
                        "Missing field '{}' in expression of kind '{}'",
                        $name,
                        $node.kind()
                    ),
                    Some($node.range()),
                ));
                None
            }
        }
    }};
}

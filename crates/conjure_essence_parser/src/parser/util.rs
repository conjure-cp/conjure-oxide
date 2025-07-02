use tree_sitter::{Node, Parser, Tree};
use tree_sitter_essence::LANGUAGE;

use super::traversal::WalkDFS;

/// Parse the given source code into a syntax tree using tree-sitter.
///
/// If successful, returns a tuple containing the syntax tree and the raw source code.
/// If the source code is not valid Essence, returns None.
///
/// NOTE: The new source code may be different from the original source code.
///       See implementation for details.
pub fn get_tree(src: &str) -> Option<(Tree, String)> {
    let mut parser = Parser::new();
    parser.set_language(&LANGUAGE.into()).unwrap();

    parser.parse(src, None).and_then(|tree| {
        let root = tree.root_node();
        if root.is_error() {
            return None;
        }

        let children: Vec<_> = named_children(&root).collect();
        let first_child = children.first()?;

        // HACK: Tree-sitter can only parse a complete program from top to bottom, not an individual bit of syntax.
        // See: https://github.com/tree-sitter/tree-sitter/issues/711 and linked issues.
        // However, we can catch the case where the top node is an error, and wrap it in a "such that" node.
        // This way we can parse an isolated expression and it is only slightly cursed :)
        if first_child.is_error() {
            if src.starts_with("such that") {
                None
            } else {
                get_tree(&format!("such that {src}"))
            }
        } else {
            Some((tree, src.to_string()))
        }
    })
}

/// Get the named children of a node
pub fn named_children<'a>(node: &'a Node<'a>) -> impl Iterator<Item = Node<'a>> + 'a {
    (0..node.named_child_count()).filter_map(|i| node.named_child(i))
}

/// Get all top-level nodes that match the given predicate
pub fn query_toplevel<'a>(
    node: &'a Node<'a>,
    predicate: &'a dyn Fn(&Node<'a>) -> bool,
) -> impl Iterator<Item = Node<'a>> + 'a {
    WalkDFS::with_retract(node, predicate).filter(|n| n.is_named() && predicate(n))
}

/// Get all meta-variable names in a node
pub fn get_metavars<'a>(node: &'a Node<'a>, src: &'a str) -> impl Iterator<Item = String> + 'a {
    query_toplevel(node, &|n| n.kind() == "metavar").filter_map(|child| {
        child
            .named_child(0)
            .map(|name| src[name.start_byte()..name.end_byte()].to_string())
    })
}

mod test {
    #[allow(unused)]
    use super::*;

    #[test]
    fn test_get_metavars() {
        let src = "such that &x = y";
        let (tree, _) = get_tree(src).unwrap();
        let root = tree.root_node();
        let metavars = get_metavars(&root, src).collect::<Vec<_>>();
        assert_eq!(metavars, vec!["x"]);
    }
}

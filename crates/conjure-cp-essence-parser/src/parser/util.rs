use tree_sitter::{Node, Parser, Tree};
use tree_sitter_essence::LANGUAGE;

use super::traversal::WalkDFS;
use crate::diagnostics::source_map::SourceMap;
use crate::errors::RecoverableParseError;
use conjure_cp_core::ast::SymbolTablePtr;

/// Context for parsing, containing shared state passed through parser functions.
pub struct ParseContext<'a> {
    pub source_code: &'a str,
    pub root: &'a Node<'a>,
    pub symbols: Option<SymbolTablePtr>,
    pub errors: &'a mut Vec<RecoverableParseError>,
    pub source_map: &'a mut SourceMap,
    pub typechecking_context: TypecheckingContext,
}

impl<'a> ParseContext<'a> {
    pub fn new(
        source_code: &'a str,
        root: &'a Node<'a>,
        symbols: Option<SymbolTablePtr>,
        errors: &'a mut Vec<RecoverableParseError>,
        source_map: &'a mut SourceMap,
    ) -> Self {
        Self {
            source_code,
            root,
            symbols,
            errors,
            source_map,
            typechecking_context: TypecheckingContext::Unknown,
        }
    }

    pub fn record_error(&mut self, error: RecoverableParseError) {
        self.errors.push(error);
    }

    /// Create a new ParseContext with different symbols but sharing source_code, root, errors, and source_map.
    pub fn with_new_symbols(&mut self, symbols: Option<SymbolTablePtr>) -> ParseContext<'_> {
        ParseContext {
            source_code: self.source_code,
            root: self.root,
            symbols,
            errors: self.errors,
            source_map: self.source_map,
            typechecking_context: self.typechecking_context,
        }
    }
}

// Used to detect type mismatches during parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypecheckingContext {
    Boolean,
    Arithmetic,
    /// Context is unknown or flexible
    Unknown,
}

/// Parse the given source code into a syntax tree using tree-sitter.
///
/// If successful, returns a tuple containing the syntax tree and the raw source code.
/// If the source code is not valid Essence, returns None.
pub fn get_tree(src: &str) -> Option<(Tree, String)> {
    parse_tree(src)
}

/// Parse an isolated expression by prefixing it with `_FRAGMENT_EXPRESSION`.
///
/// NOTE: The returned source code includes the injected prefix so that node ranges remain valid.
pub fn get_tree_fragment(src: &str) -> Option<(Tree, String)> {
    let prefixed = format!("_FRAGMENT_EXPRESSION {src}");
    parse_tree(&prefixed)
}

fn parse_tree(src: &str) -> Option<(Tree, String)> {
    let mut parser = Parser::new();
    parser.set_language(&LANGUAGE.into()).unwrap();

    parser.parse(src, None).and_then(|tree| {
        if tree.root_node().is_error() {
            return None;
        }
        Some((tree, src.to_string()))
    })
}

/// Get the named children of a node
pub fn named_children<'a>(node: &'a Node<'a>) -> impl Iterator<Item = Node<'a>> + 'a {
    (0..node.named_child_count())
        .filter_map(|i| u32::try_from(i).ok().and_then(|i| node.named_child(i)))
}

pub fn node_is_expression(node: &Node) -> bool {
    matches!(
        node.kind(),
        "bool_expr" | "arithmetic_expr" | "comparison_expr" | "atom"
    )
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

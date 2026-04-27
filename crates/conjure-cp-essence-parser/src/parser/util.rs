use std::collections::BTreeMap;

use tree_sitter::{Node, Parser, Tree};
use tree_sitter_essence::LANGUAGE;

use super::traversal::WalkDFS;
use crate::diagnostics::diagnostics_api::SymbolKind;
use crate::diagnostics::source_map::{HoverInfo, SourceMap, SpanId, span_with_hover};
use crate::errors::RecoverableParseError;
use conjure_cp_core::ast::{Name, SymbolTablePtr};

/// Context for parsing, containing shared state passed through parser functions.
pub struct ParseContext<'a> {
    pub source_code: &'a str,
    pub root: &'a Node<'a>,
    pub symbols: Option<SymbolTablePtr>,
    pub errors: &'a mut Vec<RecoverableParseError>,
    pub source_map: &'a mut SourceMap,
    pub decl_spans: &'a mut BTreeMap<Name, SpanId>,
    /// What type the current expression/literal itself should be
    pub typechecking_context: TypecheckingContext,
    /// What type the elements within a collection should be
    pub inner_typechecking_context: TypecheckingContext,
}

impl<'a> ParseContext<'a> {
    pub fn new(
        source_code: &'a str,
        root: &'a Node<'a>,
        symbols: Option<SymbolTablePtr>,
        errors: &'a mut Vec<RecoverableParseError>,
        source_map: &'a mut SourceMap,
        decl_spans: &'a mut BTreeMap<Name, SpanId>,
    ) -> Self {
        Self {
            source_code,
            root,
            symbols,
            errors,
            source_map,
            decl_spans,
            typechecking_context: TypecheckingContext::Unknown,
            inner_typechecking_context: TypecheckingContext::Unknown,
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
            decl_spans: self.decl_spans,
            typechecking_context: self.typechecking_context,
            inner_typechecking_context: self.inner_typechecking_context,
        }
    }

    pub fn save_decl_span(&mut self, name: Name, span_id: SpanId) {
        self.decl_spans.insert(name, span_id);
    }

    pub fn lookup_decl_span(&self, name: &Name) -> Option<SpanId> {
        self.decl_spans.get(name).copied()
    }

    pub fn lookup_decl_line(&self, name: &Name) -> Option<u32> {
        let span_id = self.lookup_decl_span(name)?;
        let span = self.source_map.spans.get(span_id as usize)?;
        Some(span.start_point.line + 1)
    }

    /// Helper to add to span and documentation hover info into the source map
    pub fn add_span_and_doc_hover(
        &mut self,
        node: &tree_sitter::Node,
        doc_key: &str, // name of the documentation file in Bits
        kind: SymbolKind,
        ty: Option<String>,
        decl_span: Option<u32>,
    ) {
        if let Some(description) = get_documentation(doc_key) {
            let hover = HoverInfo {
                description,
                kind: Some(kind),
                ty,
                decl_span,
            };
            span_with_hover(node, self.source_code, self.source_map, hover);
        }
        // If documentation is not found, do nothing (no fallback, no addition to source map)
    }
}

// Used to detect type mismatches during parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypecheckingContext {
    Boolean,
    Arithmetic,
    Set,
    SetOrMatrix,
    MSet,
    Matrix,
    Tuple,
    Record,
    /// Context is unknown or flexible
    Unknown,
}

/// Parse the given source code into a syntax tree using tree-sitter.
///
/// If successful, returns a tuple containing the syntax tree and the raw source code.
/// If the source code is not valid Essence, returns None.
pub fn get_tree(src: &str) -> Option<(Tree, String)> {
    let mut parser = Parser::new();
    parser.set_language(&LANGUAGE.into()).unwrap();

    parser.parse(src, None).and_then(|tree| {
        let root = tree.root_node();
        if root.is_error() {
            return None;
        }
        Some((tree, src.to_string()))
    })
}

/// Parse an expression fragment, allowing a dummy prefix for error recovery.
///
/// NOTE: The new source code may be different from the original source code.
///       See implementation for details.
pub fn get_expr_tree(src: &str) -> Option<(Tree, String)> {
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
        // However, we can use a dummy _FRAGMENT_EXPRESSION prefix (which we insert as necessary)
        // to trick the parser into accepting an isolated expression.
        // This way we can parse an isolated expression and it is only slightly cursed :)
        if first_child.is_error() {
            if src.starts_with("_FRAGMENT_EXPRESSION") {
                None
            } else {
                get_expr_tree(&format!("_FRAGMENT_EXPRESSION {src}"))
            }
        } else {
            Some((tree, src.to_string()))
        }
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

/// Fetch Essence syntax documentation from Conjure's `docs/bits/` folder on GitHub.
///
/// `name` is the name of the documentation file (without .md suffix). If the file is not found or an error occurs, returns None.
pub fn get_documentation(name: &str) -> Option<String> {
    let mut base = name.to_string();
    if let Some(stripped) = base.strip_suffix(".md") {
        base = stripped.to_string();
    }

    // This url is for raw Markdown bytes
    let url =
        format!("https://raw.githubusercontent.com/conjure-cp/conjure/main/docs/bits/{base}.md");

    let output = std::process::Command::new("curl")
        .args(["-fsSL", &url])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout).ok()
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

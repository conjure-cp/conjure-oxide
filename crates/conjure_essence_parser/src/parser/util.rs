#![allow(clippy::legacy_numeric_constants)]
use std::fs;

use tree_sitter::{Node, Parser, Tree};
use tree_sitter_essence::LANGUAGE;

/// Read an Essence file and return a tuple of (AST, source)
/// Where:
/// - AST is the syntax tree as parsed by tree-sitter
/// - source is the raw source code as a string
pub fn read_essence_file(path: &str, filename: &str, extension: &str) -> (Tree, String) {
    let pth = format!("{path}/{filename}.{extension}");
    let source_code = fs::read_to_string(&pth)
        .unwrap_or_else(|_| panic!("Failed to read the source code file {}", pth));
    (get_tree(&source_code), source_code)
}

/// Parse the given source code into a syntax tree using tree-sitter
pub fn get_tree(src: &str) -> Tree {
    let mut parser = Parser::new();
    parser.set_language(&LANGUAGE.into()).unwrap();
    parser
        .parse(src.to_string(), None)
        .expect("Failed to parse")
}

/// Get the named children of a node
pub fn named_children<'a>(node: &'a Node<'a>) -> impl Iterator<Item = Node<'a>> + 'a {
    (0..node.named_child_count()).filter_map(|i| node.named_child(i))
}

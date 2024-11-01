use libloading::{Library, Symbol};
use std::fs;
use std::path::Path;
use tree_sitter::{Language, Node, Parser, Tree};
use tree_sitter_essence::LANGUAGE;

fn main() {
    let tree = get_tree("./test_code.txt");
    let root_node = tree.root_node();
    print_tree(root_node, 0);
}

fn get_tree(source_code_path_str: &str) -> Tree {
    let mut parser = Parser::new();
    parser.set_language(&LANGUAGE.into()).unwrap();

    let source_code_path = Path::new(source_code_path_str);
    let source_code =
        fs::read_to_string(source_code_path).expect("Failed to read the source code file");
    let tree = parser.parse(source_code, None).expect("Failed to parse");

    return tree;
}

fn print_tree(node: tree_sitter::Node, indent: usize) {
    let prefix = "  ".repeat(indent);
    let kind = node.kind();
    let start_byte = node.start_byte();
    let end_byte = node.end_byte();
    //let content = &source[start_byte..end_byte];

    println!("{}{}: {} - {}", prefix, kind, start_byte, end_byte);

    for child in node.children(&mut node.walk()) {
        print_tree(child, indent + 1);
    }
}

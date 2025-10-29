use std::fs;
use std::sync::{Arc, RwLock};

use conjure_cp_core::Model;
use conjure_cp_core::ast::Expression;
use conjure_cp_core::ast::Metadata;
use conjure_cp_core::ast::{DeclarationPtr, Moo};
use conjure_cp_core::context::Context;
#[allow(unused)]
use uniplate::Uniplate;

use super::find::parse_find_statement;
use super::letting::parse_letting_statement;
use super::util::{get_tree, named_children};
use crate::errors::EssenceParseError;
use crate::expression::parse_expression;
use tree_sitter::Node;

/// Parse an Essence file into a Model using the tree-sitter parser.
pub fn parse_essence_file_native(
    path: &str,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Model, EssenceParseError> {
    let source_code = fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("Failed to read the source code file {path}"));
    parse_essence_with_context(&source_code, context)
}

// reserved keywords list for the 'keyword as var' error
const RESERVED_KEYWORDS: &[&str] = &[
    "find",
    "letting",
    "be",
    "domain",
    "true",
    "false",
    "bool",
    "int",
    "and",
    "or",
    "min",
    "max",
    "sum",
    "allDiff",
    "toInt",
    "fromSolution",
    "dominanceRelation",
];

pub fn parse_essence_with_context(
    src: &str,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Model, EssenceParseError> {
    let (tree, source_code) = match get_tree(src) {
        Some(tree) => tree,
        None => {
            return Err(EssenceParseError::TreeSitterError(
                "Failed to parse source code".to_string(),
            ));
        }
    };

    let mut model = Model::new(context);
    // let symbols = model.as_submodel().symbols().clone();
    let root_node = tree.root_node();

    // the keyword as var check
    if let Some((node, ident)) = find_keyword_as_variable(root_node, &source_code) {
        let pos = node.start_position();

        // better error message later on?
        return Err(EssenceParseError::syntax_error(
            format!(
                "'{}' is a keyword and cannot be used as variable at line {}",
                ident,
                pos.row + 1,
            ),
            Some(node.range()),
        ));
    }

    // Early syntax gate: if the parse contains errors or missing nodes, return a helpful error.
    if root_node.has_error() {
        if let Some(bad) = first_syntax_issue(root_node) {
            let pos = bad.start_position();
            return Err(EssenceParseError::syntax_error(
                format!(
                    "Syntax error near '{}' at line {}, column {}",
                    bad.kind(),
                    pos.row + 1,
                    pos.column + 1
                ),
                None,
            ));
        }
        return Err(EssenceParseError::syntax_error(
            "Syntax error".to_string(),
            None,
        ));
    }
    for statement in named_children(&root_node) {
        match statement.kind() {
            "single_line_comment" => {}
            "language_declaration" => {}
            "find_statement_list" => {
                let var_hashmap = parse_find_statement(statement, &source_code);
                for (name, domain) in var_hashmap {
                    model
                        .as_submodel_mut()
                        .symbols_mut()
                        .insert(DeclarationPtr::new_var(name, domain));
                }
            }
            "constraint_list" => {
                let mut constraint_vec: Vec<Expression> = Vec::new();
                for constraint in named_children(&statement) {
                    let current_symbols = model.as_submodel().symbols().clone();

                    if constraint.kind() != "single_line_comment" {
                        constraint_vec.push(parse_expression(
                            constraint,
                            &source_code,
                            &statement,
                            Some(&current_symbols),
                        )?);
                    }
                }
                model.as_submodel_mut().add_constraints(constraint_vec);
            }
            "language_label" => {}
            "letting_statement_list" => {
                let letting_vars = parse_letting_statement(statement, &source_code)?;
                model.as_submodel_mut().symbols_mut().extend(letting_vars);
            }
            "dominance_relation" => {
                let inner = statement
                    .child(1)
                    .expect("Expected a sub-expression inside `dominanceRelation`");
                let current_symbols = model.as_submodel().symbols().clone();
                let expr =
                    parse_expression(inner, &source_code, &statement, Some(&current_symbols))?;
                let dominance = Expression::DominanceRelation(Metadata::new(), Moo::new(expr));
                if model.dominance.is_some() {
                    return Err(EssenceParseError::syntax_error(
                        "Duplicate dominance relation".to_string(),
                        None,
                    ));
                }
                model.dominance = Some(dominance);
            }
            _ => {
                let kind = statement.kind();
                return Err(EssenceParseError::syntax_error(
                    format!("Unrecognized top level statement kind: {kind}"),
                    None,
                ));
            }
        }
    }
    Ok(model)
}

/// Find the first error or missing node in a subtree (preorder DFS)
fn first_syntax_issue(root: Node) -> Option<Node> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.is_error() || node.is_missing() {
            return Some(node);
        }
        let count = node.child_count();
        for i in (0..count).rev() {
            if let Some(child) = node.child(i) {
                stack.push(child);
            }
        }
    }
    None
}

// keyword as var check: walk the tree and find vars as keywords
fn find_keyword_as_variable<'a>(root: Node<'a>, src: &str) -> Option<(Node<'a>, String)> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        // we first check variable nodes whose text is a key word
        if node.kind() == "variable" {
            if let Ok(text) = node.utf8_text(src.as_bytes()) {
                let ident = text.trim();
                if RESERVED_KEYWORDS.contains(&ident) {
                    return Some((node, ident.to_string()));
                }
            }
        }

        // some keywords, as defined by the grammar, can be parsed keyword token (ends with _kw) immediately followed by an ERROR node
        // This catches cases like "find find,..." where the second "find" causes a parse error. because this rule has precedence over the variable rule
        if node.kind().ends_with("_kw") || node.kind().ends_with("_statement_list") {
            if let Some(next) = node.next_sibling() {
                if next.is_error() {
                    // now the keyword that caused the problem is the current node
                    // if the next one is an error node
                    if let Ok(kw_text) = node.utf8_text(src.as_bytes()) {
                        let kw = kw_text.trim();
                        if RESERVED_KEYWORDS.contains(&kw) {
                            return Some((next, kw.to_string()));
                        }
                    }
                }
            }
        }

        // push children
        for i in (0..node.child_count()).rev() {
            if let Some(child) = node.child(i) {
                stack.push(child);
            }
        }
    }
    None
}

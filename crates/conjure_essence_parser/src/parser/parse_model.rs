use std::fs;
use std::rc::Rc;
use std::sync::{Arc, RwLock};

use conjure_core::ast::Declaration;
use conjure_core::ast::Expression;
use conjure_core::context::Context;
use conjure_core::error::Error;
use conjure_core::metadata::Metadata;
use conjure_core::Model;
#[allow(unused)]
use uniplate::Uniplate;

use crate::errors::EssenceParseError;

use super::expression::parse_expression;
use super::find::parse_find_statement;
use super::letting::parse_letting_statement;
use super::util::{get_tree, named_children};

/// Parse an Essence file into a Model using the tree-sitter parser.
pub fn parse_essence_file_native(
    path: &str,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Model, EssenceParseError> {
    let source_code = fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("Failed to read the source code file {}", path));
    parse_essence_with_context(&source_code, context)
}

pub fn parse_essence_with_context(
    src: &str,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Model, EssenceParseError> {
    let (tree, source_code) = match get_tree(src) {
        Some(tree) => tree,
        None => {
            return Err(EssenceParseError::TreeSitterError(
                "Failed to parse source code".to_string(),
            ))
        }
    };

    let mut model = Model::new(context);
    let root_node = tree.root_node();
    for statement in named_children(&root_node) {
        match statement.kind() {
            "single_line_comment" => {}
            "find_statement" => {
                let var_hashmap = parse_find_statement(statement, &source_code);
                for (name, decision_variable) in var_hashmap {
                    model
                        .as_submodel_mut()
                        .symbols_mut()
                        .insert(Rc::new(Declaration::new_var(name, decision_variable)));
                }
            }
            "bool_expr" | "atom" | "comparison_expr" => {
                model.as_submodel_mut().add_constraint(parse_expression(
                    statement,
                    &source_code,
                    &statement,
                )?);
            }
            "language_label" => {}
            "letting_statement" => {
                let letting_vars = parse_letting_statement(statement, &source_code)?;
                model.as_submodel_mut().symbols_mut().extend(letting_vars);
            }
            "dominance_relation" => {
                let inner = statement
                    .child_by_field_name("expression")
                    .expect("Expected a sub-expression inside `dominanceRelation`");
                let expr = parse_expression(inner, &source_code, &statement)?;
                let dominance = Expression::DominanceRelation(Metadata::new(), Box::new(expr));
                if model.dominance.is_some() {
                    return Err(EssenceParseError::ParseError(Error::Parse(
                        "Duplicate dominance relation".to_owned(),
                    )));
                }
                model.dominance = Some(dominance);
            }
            _ => {
                let kind = statement.kind();
                return Err(EssenceParseError::ParseError(Error::Parse(
                    format!("Unrecognized top level statement kind: {kind}").to_owned(),
                )));
            }
        }
    }
    Ok(model)
}

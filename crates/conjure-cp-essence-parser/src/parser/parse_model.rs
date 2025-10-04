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

/// Parse an Essence file into a Model using the tree-sitter parser.
pub fn parse_essence_file_native(
    path: &str,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Model, EssenceParseError> {
    let source_code = fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("Failed to read the source code file {path}"));
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
            ));
        }
    };

    let mut model = Model::new(context);
    // let symbols = model.as_submodel().symbols().clone();
    let root_node = tree.root_node();
    for statement in named_children(&root_node) {
        match statement.kind() {
            "single_line_comment" => {}
            "find_statement" => {
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
            "letting_statement" => {
                let letting_vars = parse_letting_statement(statement, &source_code)?;
                model.as_submodel_mut().symbols_mut().extend(letting_vars);
            }
            "dominance_relation" => {
                let inner = statement
                    .child_by_field_name("expression")
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

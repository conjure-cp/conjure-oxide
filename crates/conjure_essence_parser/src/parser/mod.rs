use std::rc::Rc;
use std::sync::{Arc, RwLock};

use conjure_core::ast::Declaration;
use conjure_core::ast::Expression;
use conjure_core::context::Context;
use conjure_core::error::Error;
use conjure_core::metadata::Metadata;
use conjure_core::Model;

use crate::errors::EssenceParseError;

mod domain;
mod expression;
mod find;
mod letting;
mod util;

use expression::parse_expression;
use find::parse_find_statement;
use letting::parse_letting_statement;
use util::{named_children, read_essence_file};

/// Parse an Essence file into a Model using the tree-sitter parser.
pub fn parse_essence_file_native(
    path: &str,
    filename: &str,
    extension: &str,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Model, EssenceParseError> {
    let (tree, source_code) = read_essence_file(path, filename, extension);

    let mut model = Model::new(context);
    let root_node = tree.root_node();
    for statement in named_children(&root_node) {
        match statement.kind() {
            "single_line_comment" => {}
            "find_statement_list" => {
                let var_hashmap = parse_find_statement(statement, &source_code);
                for (name, decision_variable) in var_hashmap {
                    model
                        .as_submodel_mut()
                        .symbols_mut()
                        .insert(Rc::new(Declaration::new_var(name, decision_variable)));
                }
            }
            "constraint_list" => {
                let mut constraint_vec: Vec<Expression> = Vec::new();
                for constraint in named_children(&statement) {
                    if constraint.kind() != "single_line_comment" {
                        constraint_vec.push(parse_expression(constraint, &source_code, &statement));
                    }
                }
                model.as_submodel_mut().add_constraints(constraint_vec);
            }
            "e_prime_label" => {}
            "letting_statement_list" => {
                let letting_vars = parse_letting_statement(statement, &source_code);
                model.as_submodel_mut().symbols_mut().extend(letting_vars);
            }
            "dominance_relation" => {
                let inner = statement
                    .child(1)
                    .expect("Expected a sub-expression inside `dominanceRelation`");
                let expr = parse_expression(inner, &source_code, &statement);
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

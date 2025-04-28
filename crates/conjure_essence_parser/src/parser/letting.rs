#![allow(clippy::legacy_numeric_constants)]
use std::collections::BTreeSet;
use std::rc::Rc;

use tree_sitter::Node;

use super::domain::parse_domain;
use super::expression::parse_expression;
use super::util::named_children;
use crate::errors::EssenceParseError;
use conjure_core::ast::Declaration;
use conjure_core::ast::{Name, SymbolTable};

/// Parse a letting statement into a SymbolTable containing the declared symbols
pub fn parse_letting_statement(
    letting_statement_list: Node,
    source_code: &str,
) -> Result<SymbolTable, EssenceParseError> {
    let mut symbol_table = SymbolTable::new();

    for letting_statement in named_children(&letting_statement_list) {
        let mut temp_symbols = BTreeSet::new();

        let variable_list = letting_statement
            .child_by_field_name("variable_list")
            .expect("No variable list found");
        for variable in named_children(&variable_list) {
            let variable_name = &source_code[variable.start_byte()..variable.end_byte()];
            temp_symbols.insert(variable_name);
        }

        let expr_or_domain = letting_statement
            .child_by_field_name("expr_or_domain")
            .expect("No domain or expression found for letting statement");
        match expr_or_domain.kind() {
            "bool_expr" | "arithmetic_expr" => {
                for name in temp_symbols {
                    symbol_table.insert(Rc::new(Declaration::new_value_letting(
                        Name::UserName(String::from(name)),
                        parse_expression(expr_or_domain, source_code, &letting_statement_list)?,
                    )));
                }
            }
            "domain" => {
                // let domain = expr_or_domain.next_sibling().expect("No domain found in letting statement");
                for name in temp_symbols {
                    symbol_table.insert(Rc::new(Declaration::new_domain_letting(
                        Name::UserName(String::from(name)),
                        parse_domain(expr_or_domain, source_code),
                    )));
                }
            }
            _ => panic!("Unrecognized node in letting statement"),
        }
    }
    Ok(symbol_table)
}

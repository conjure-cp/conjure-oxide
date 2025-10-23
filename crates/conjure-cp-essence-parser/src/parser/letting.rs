#![allow(clippy::legacy_numeric_constants)]
use std::collections::BTreeSet;

use tree_sitter::Node;

use super::domain::parse_domain;
use super::util::named_children;
use crate::errors::EssenceParseError;
use crate::expression::parse_expression;
use conjure_cp_core::ast::DeclarationPtr;
use conjure_cp_core::ast::{Name, SymbolTable};

/// Parse a letting statement into a SymbolTable containing the declared symbols
pub fn parse_letting_statement(
    letting_statement: Node,
    source_code: &str,
    existing_symbols: Option<&SymbolTable>,
) -> Result<SymbolTable, EssenceParseError> {
    let mut symbol_table = SymbolTable::new();

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
                symbol_table.insert(DeclarationPtr::new_value_letting(
                    Name::user(name),
                    parse_expression(
                        expr_or_domain,
                        source_code,
                        &letting_statement,
                        existing_symbols,
                    )?,
                ));
            }
        }
        "domain" => {
            for name in temp_symbols {
                let domain = parse_domain(expr_or_domain, source_code)?;

                // If it's a record domain, add the field names to the symbol table
                if let conjure_cp_core::ast::Domain::Record(ref entries) = domain {
                    for entry in entries {
                        // Add each field name as a record field declaration
                        symbol_table.insert(DeclarationPtr::new_record_field(entry.clone()));
                    }
                }

                symbol_table.insert(DeclarationPtr::new_domain_letting(Name::user(name), domain));
            }
        }
        _ => {
            return Err(EssenceParseError::syntax_error(
                format!(
                    "Expected letting expression, got '{}'",
                    expr_or_domain.kind()
                ),
                Some(expr_or_domain.range()),
            ));
        }
    }

    Ok(symbol_table)
}

#![allow(clippy::legacy_numeric_constants)]
use std::collections::BTreeSet;

use tree_sitter::Node;

use super::domain::parse_domain;
use super::util::named_children;
use crate::diagnostics::diagnostics_api::SymbolKind;
use crate::diagnostics::source_map::{HoverInfo, SourceMap, span_with_hover};
use crate::errors::{FatalParseError, RecoverableParseError};
use crate::expression::parse_expression;
use crate::field;
use conjure_cp_core::ast::DeclarationPtr;
use conjure_cp_core::ast::{Name, SymbolTable, SymbolTablePtr};

/// Parse a letting statement into a SymbolTable containing the declared symbols
pub fn parse_letting_statement(
    letting_statement: Node,
    source_code: &str,
    existing_symbols_ptr: Option<SymbolTablePtr>,
    errors: &mut Vec<RecoverableParseError>,
    source_map: &mut SourceMap,
) -> Result<Option<SymbolTable>, FatalParseError> {
    let mut symbol_table = SymbolTable::new();

    let mut temp_symbols = BTreeSet::new();

    let variable_list = field!(letting_statement, "variable_list");
    for variable in named_children(&variable_list) {
        let variable_name = &source_code[variable.start_byte()..variable.end_byte()];
        temp_symbols.insert(variable_name);
        let hover = HoverInfo {
            description: format!("Letting variable: {variable_name}"),
            kind: Some(SymbolKind::Letting),
            ty: None,
            decl_span: None,
        };
        span_with_hover(&variable, source_code, source_map, hover);
    }

    let expr_or_domain = field!(letting_statement, "expr_or_domain");
    match expr_or_domain.kind() {
        "bool_expr" | "arithmetic_expr" | "atom" => {
            for name in temp_symbols {
                let Some(expr) = parse_expression(
                    expr_or_domain,
                    source_code,
                    &letting_statement,
                    existing_symbols_ptr.clone(),
                    errors,
                    source_map,
                )?
                else {
                    continue;
                };
                symbol_table.insert(DeclarationPtr::new_value_letting(Name::user(name), expr));
            }
        }
        "domain" => {
            for name in temp_symbols {
                let Some(domain) = parse_domain(
                    expr_or_domain,
                    source_code,
                    existing_symbols_ptr.clone(),
                    errors,
                    source_map,
                )?
                else {
                    continue;
                };

                // If it's a record domain, add the field names to the symbol table
                if let Some(entries) = domain.as_record() {
                    for entry in entries {
                        // Add each field name as a record field declaration
                        symbol_table.insert(DeclarationPtr::new_record_field(entry.clone()));
                    }
                }

                symbol_table.insert(DeclarationPtr::new_domain_letting(Name::user(name), domain));
            }
        }
        _ => {
            return Err(FatalParseError::internal_error(
                format!(
                    "Expected letting expression, got '{}'",
                    expr_or_domain.kind()
                ),
                Some(expr_or_domain.range()),
            ));
        }
    }

    Ok(Some(symbol_table))
}

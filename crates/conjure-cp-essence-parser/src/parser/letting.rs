#![allow(clippy::legacy_numeric_constants)]
use crate::field;
use std::collections::BTreeSet;
use tree_sitter::Node;

use super::ParseContext;
use super::domain::parse_domain;
use super::util::named_children;
use crate::diagnostics::diagnostics_api::SymbolKind;
use crate::diagnostics::source_map::{HoverInfo, span_with_hover};
use crate::errors::{FatalParseError, RecoverableParseError};
use crate::expression::parse_expression;
use conjure_cp_core::ast::DeclarationPtr;
use conjure_cp_core::ast::{Name, SymbolTable};

/// Parse a letting statement into a SymbolTable containing the declared symbols
pub fn parse_letting_statement(
    ctx: &mut ParseContext,
    letting_statement: Node,
) -> Result<Option<SymbolTable>, FatalParseError> {
    let Some(keyword) = field!(recover, ctx, letting_statement, "letting_keyword") else {
        return Ok(None);
    };
    span_with_hover(
        &keyword,
        ctx.source_code,
        ctx.source_map,
        HoverInfo {
            description: "Letting keyword".to_string(),
            kind: Some(SymbolKind::Letting),
            ty: None,
            decl_span: None,
        },
    );

    let mut symbol_table = SymbolTable::new();

    for variable_decl in named_children(&letting_statement) {
        let mut temp_symbols = BTreeSet::new();

        let Some(variable_list) = field!(recover, ctx, variable_decl, "variable_list") else {
            return Ok(None);
        };
        for variable in named_children(&variable_list) {
            let variable_name = &ctx.source_code[variable.start_byte()..variable.end_byte()];

            // Check for duplicate within the same statement
            if temp_symbols.contains(variable_name) {
                ctx.errors.push(RecoverableParseError::new(
                    format!(
                        "Variable '{}' is already declared in this letting statement",
                        variable_name
                    ),
                    Some(variable.range()),
                ));
                // don't return here, as we can still add the other variables to the symbol table
                continue;
            }

            // Check for duplicate declaration across statements
            let name = Name::user(variable_name);
            if let Some(symbols) = &ctx.symbols
                && symbols.read().lookup(&name).is_some()
            {
                let previous_line = ctx.lookup_decl_line(&name);
                ctx.errors.push(RecoverableParseError::new(
                    match previous_line {
                        Some(line) => format!(
                            "Variable '{}' is already declared in a previous statement on line {}",
                            variable_name, line
                        ),
                        None => format!(
                            "Variable '{}' is already declared in a previous statement",
                            variable_name
                        ),
                    },
                    Some(variable.range()),
                ));
                // don't return here, as we can still add the other variables to the symbol table
                continue;
            }

            temp_symbols.insert(variable_name);
            let hover = HoverInfo {
                description: format!("Letting variable: {variable_name}"),
                kind: Some(SymbolKind::LettingVar),
                ty: None,
                decl_span: None,
            };
            let span_id = span_with_hover(&variable, ctx.source_code, ctx.source_map, hover);
            ctx.save_decl_span(name, span_id);
        }

        let Some(expr_or_domain) = field!(recover, ctx, variable_decl, "expr_or_domain") else {
            return Ok(None);
        };

        if variable_decl.child_by_field_name("domain").is_some() {
            for name in temp_symbols {
                let Some(domain) = parse_domain(ctx, expr_or_domain)? else {
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
        } else {
            for name in temp_symbols {
                let Some(expr) = parse_expression(ctx, expr_or_domain)? else {
                    continue;
                };
                symbol_table.insert(DeclarationPtr::new_value_letting(Name::user(name), expr));
            }
        }
    }

    Ok(Some(symbol_table))
}

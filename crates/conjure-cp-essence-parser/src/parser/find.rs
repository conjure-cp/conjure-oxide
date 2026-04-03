#![allow(clippy::legacy_numeric_constants)]

use std::collections::BTreeMap;
use tree_sitter::Node;

use super::ParseContext;
use super::domain::parse_domain;
use super::util::named_children;
use crate::diagnostics::diagnostics_api::SymbolKind;
use crate::diagnostics::source_map::{HoverInfo, span_with_hover};
use crate::errors::{FatalParseError, RecoverableParseError};
use crate::field;
use conjure_cp_core::ast::{DomainPtr, Name};

pub fn parse_find_statement(
    ctx: &mut ParseContext,
    find_statement: Node,
) -> Result<BTreeMap<Name, DomainPtr>, FatalParseError> {
    let keyword = field!(find_statement, "find_keyword");
    span_with_hover(
        &keyword,
        ctx.source_code,
        ctx.source_map,
        HoverInfo {
            description: "Find keyword".to_string(),
            kind: Some(SymbolKind::Find),
            ty: None,
            decl_span: None,
        },
    );
    let mut var_hashmap = BTreeMap::new();
    for var_decl in named_children(&find_statement) {
        if let Ok(mut decls) = parse_declaration_statement(ctx, var_decl, SymbolKind::Find) {
            var_hashmap.append(&mut decls);
        }
    }
    Ok(var_hashmap)
}

pub fn parse_given_statement(
    ctx: &mut ParseContext,
    given_statement: Node,
) -> Result<BTreeMap<Name, DomainPtr>, FatalParseError> {
    let keyword = field!(given_statement, "given_keyword");
    span_with_hover(
        &keyword,
        ctx.source_code,
        ctx.source_map,
        HoverInfo {
            description: "Given keyword".to_string(),
            kind: Some(SymbolKind::Given),
            ty: None,
            decl_span: None,
        },
    );

    let mut var_hashmap = BTreeMap::new();
    for var_decl in named_children(&given_statement) {
        if let Ok(mut decls) = parse_declaration_statement(ctx, var_decl, SymbolKind::Given) {
            var_hashmap.append(&mut decls);
        }
    }
    Ok(var_hashmap)
}

pub fn parse_declaration_statement(
    ctx: &mut ParseContext,
    statement_node: Node,
    symbol_kind: SymbolKind,
) -> Result<BTreeMap<Name, DomainPtr>, FatalParseError> {
    let mut vars = BTreeMap::new();

    let domain = field!(statement_node, "domain");
    let Some(domain) = parse_domain(ctx, domain)? else {
        return Ok(vars);
    };

    let variable_list = field!(statement_node, "variables");
    for variable in named_children(&variable_list) {
        let variable_name = &ctx.source_code[variable.start_byte()..variable.end_byte()];
        let name = Name::user(variable_name);

        // Check for duplicate within the same statement
        if vars.contains_key(&name) {
            ctx.errors.push(RecoverableParseError::new(
                format!(
                    "Variable '{}' is already declared in this {} statement",
                    variable_name,
                    match symbol_kind {
                        SymbolKind::Find => "find",
                        SymbolKind::Given => "given",
                        _ => "declaration",
                    }
                ),
                Some(variable.range()),
            ));
            // don't return here, as we can still add the other variables to the symbol table
            continue;
        }

        // Check for duplicate declaration across statements
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

        vars.insert(name.clone(), domain.clone());
        let hover = HoverInfo {
            description: format!(
                "{} variable: {variable_name}",
                match symbol_kind {
                    SymbolKind::Find => "Find",
                    SymbolKind::Given => "Given",
                    _ => "Declaration",
                }
            ),
            kind: Some(symbol_kind),
            ty: Some(domain.to_string()),
            decl_span: None,
        };
        let span_id = span_with_hover(&variable, ctx.source_code, ctx.source_map, hover);
        ctx.save_decl_span(name, span_id);
    }

    Ok(vars)
}

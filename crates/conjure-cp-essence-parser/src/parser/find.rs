#![allow(clippy::legacy_numeric_constants)]

use std::collections::BTreeMap;
use tree_sitter::Node;

use super::ParseContext;
use super::domain::parse_domain;
use super::util::named_children;
use crate::diagnostics::diagnostics_api::SymbolKind;
use crate::diagnostics::source_map::{HoverInfo, span_with_hover};
use crate::errors::{FatalParseError, RecoverableParseError};
use conjure_cp_core::ast::{DomainPtr, Name};

pub fn parse_find_statement(
    ctx: &mut ParseContext,
    find_statement: Node,
) -> Result<BTreeMap<Name, DomainPtr>, FatalParseError> {
    let keyword = find_statement.child_by_field_name("find_keyword");
    let Some(keyword) = keyword else {
        ctx.record_error(RecoverableParseError::new(
            "Missing find keyword".to_string(),
            Some(find_statement.range()),
        ));
        return Ok(BTreeMap::new());
    };
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
    let keyword = given_statement.child_by_field_name("given_keyword");
    let Some(keyword) = keyword else {
        ctx.record_error(RecoverableParseError::new(
            "Missing given keyword".to_string(),
            Some(given_statement.range()),
        ));
        return Ok(BTreeMap::new());
    };
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

    let domain_node = statement_node.child_by_field_name("domain");
    let Some(domain_node) = domain_node else {
        ctx.record_error(RecoverableParseError::new(
            "Missing domain in find statement".to_string(),
            Some(statement_node.range()),
        ));
        return Ok(vars);
    };

    let Some(domain) = parse_domain(ctx, domain_node)? else {
        return Ok(vars);
    };

    let variable_list = statement_node.child_by_field_name("variables");
    let Some(variable_list) = variable_list else {
        ctx.record_error(RecoverableParseError::new(
            "Missing variable list in find statement".to_string(),
            Some(statement_node.range()),
        ));
        return Ok(vars);
    };
    for variable in named_children(&variable_list) {
        // avoid the _FRAGMENT_EXPRESSION panic by checking range before slicing the source code
        let start = variable.start_byte();
        let end = variable.end_byte();
        if end > ctx.source_code.len() {
            ctx.record_error(RecoverableParseError::new(
                "Variable name extends beyond end of source code".to_string(),
                Some(variable.range()),
            ));
            continue;
        }
        let variable_name = &ctx.source_code[start..end];
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

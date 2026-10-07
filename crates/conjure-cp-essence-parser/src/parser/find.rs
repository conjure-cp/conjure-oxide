#![allow(clippy::legacy_numeric_constants)]
use crate::field;

use std::collections::BTreeMap;
use tree_sitter::Node;

use super::ParseContext;
use super::domain::parse_domain;
use super::util::named_children;
use crate::diagnostics::diagnostics_api::SymbolKind;
use crate::diagnostics::source_map::{HoverInfo, span_with_hover};
use crate::errors::{FatalParseError, RecoverableParseError};
use crate::parser::keyword_checks::is_keyword_identifier;
use conjure_cp_core::ast::{DomainPtr, Name};

/// Parsed find / findAux declarations and whether they are auxiliary.
pub struct ParsedFindStatement {
    pub declarations: BTreeMap<Name, DomainPtr>,
    pub auxiliary: bool,
}

pub fn parse_find_statement(
    ctx: &mut ParseContext,
    find_statement: Node,
) -> Result<ParsedFindStatement, FatalParseError> {
    let Some(keyword) = field!(recover, ctx, find_statement, "find_keyword") else {
        return Ok(ParsedFindStatement {
            declarations: BTreeMap::new(),
            auxiliary: false,
        });
    };

    let keyword_text = &ctx.source_code[keyword.start_byte()..keyword.end_byte()];
    let auxiliary = keyword_text == "findAux";
    let (doc_key, symbol_kind, var_kind) = if auxiliary {
        ("findAux", SymbolKind::Find, SymbolKind::FindVar)
    } else {
        ("find", SymbolKind::Find, SymbolKind::FindVar)
    };
    ctx.add_span_and_doc_hover(&keyword, doc_key, symbol_kind, None, None);

    let mut declarations = BTreeMap::new();
    for var_decl in named_children(&find_statement) {
        if let Ok(mut decls) = parse_declaration_statement(ctx, var_decl, var_kind, auxiliary) {
            declarations.append(&mut decls);
        }
    }
    Ok(ParsedFindStatement {
        declarations,
        auxiliary,
    })
}

pub fn parse_given_statement(
    ctx: &mut ParseContext,
    given_statement: Node,
) -> Result<BTreeMap<Name, DomainPtr>, FatalParseError> {
    let Some(keyword) = field!(recover, ctx, given_statement, "given_keyword") else {
        return Ok(BTreeMap::new());
    };
    span_with_hover(
        &keyword,
        ctx.source_code,
        ctx.source_map,
        HoverInfo {
            description: "Given keyword".to_string(),
            doc_key: None,
            kind: Some(SymbolKind::Given),
            ty: None,
            decl_span: None,
        },
    );

    let mut var_hashmap = BTreeMap::new();
    for var_decl in named_children(&given_statement) {
        if let Ok(mut decls) =
            parse_declaration_statement(ctx, var_decl, SymbolKind::GivenVar, false)
        {
            var_hashmap.append(&mut decls);
        }
    }
    Ok(var_hashmap)
}

pub fn parse_declaration_statement(
    ctx: &mut ParseContext,
    statement_node: Node,
    symbol_kind: SymbolKind,
    auxiliary_find: bool,
) -> Result<BTreeMap<Name, DomainPtr>, FatalParseError> {
    let mut vars = BTreeMap::new();

    let Some(domain_node) = field!(recover, ctx, statement_node, "domain") else {
        return Ok(vars);
    };

    let Some(domain) = parse_domain(ctx, domain_node)? else {
        return Ok(vars);
    };

    let Some(variable_list) = field!(recover, ctx, statement_node, "variables") else {
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

        if is_keyword_identifier(variable_name) {
            ctx.errors.push(RecoverableParseError::new(
                format!("Keyword '{variable_name}' used as identifier"),
                Some(variable.range()),
            ));
            // still add variable to symbol table to avoid follow-up errors
        }

        // Check for duplicate within the same statement
        if vars.contains_key(&name) {
            ctx.errors.push(RecoverableParseError::new(
                format!(
                    "Variable '{}' is already declared in this {} statement",
                    variable_name,
                    match symbol_kind {
                        SymbolKind::FindVar if auxiliary_find => "findAux",
                        SymbolKind::FindVar => "find",
                        SymbolKind::GivenVar => "given",
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
                    SymbolKind::FindVar if auxiliary_find => "FindAux",
                    SymbolKind::FindVar => "Find",
                    SymbolKind::GivenVar => "Given",
                    _ => "Declaration",
                }
            ),
            doc_key: None,
            kind: Some(symbol_kind),
            ty: Some(domain.to_string()),
            decl_span: None,
        };
        let span_id = span_with_hover(&variable, ctx.source_code, ctx.source_map, hover);
        ctx.save_decl_span(name, span_id);
    }

    Ok(vars)
}

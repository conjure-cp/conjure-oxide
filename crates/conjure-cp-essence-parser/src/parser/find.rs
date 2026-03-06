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

/// Parse a find statement into a map of decision variable names to their domains.
pub fn parse_find_statement(
    ctx: &mut ParseContext,
    find_statement: Node,
) -> Result<BTreeMap<Name, DomainPtr>, FatalParseError> {
    let mut vars = BTreeMap::new();

    let domain = field!(find_statement, "domain");
    let Some(domain) = parse_domain(ctx, domain)? else {
        return Ok(vars);
    };

    let variable_list = field!(find_statement, "variables");
    for variable in named_children(&variable_list) {
        let variable_name = &ctx.source_code[variable.start_byte()..variable.end_byte()];
        let name = Name::user(variable_name);

        // Check for duplicate within the same statement
        if vars.contains_key(&name) {
            ctx.errors.push(RecoverableParseError::new(
                format!(
                    "Variable '{}' is already declared in this find statement",
                    variable_name
                ),
                Some(variable.range()),
            ));
            // don't return here, as we can still add the other variables to the symbol table
            continue;
        }

        // Check for duplicate declaration across statements
        if let Some(symbols) = &ctx.symbols {
            if symbols.read().lookup(&name).is_some() {
                ctx.errors.push(RecoverableParseError::new(
                    format!(
                        "Variable '{}' is already declared in a previous statement",
                        variable_name
                    ),
                    Some(variable.range()),
                ));
                // don't return here, as we can still add the other variables to the symbol table
                continue;
            }
        }

        vars.insert(name, domain.clone());
        let hover = HoverInfo {
            description: format!("Find variable: {variable_name}"),
            kind: Some(SymbolKind::Find),
            ty: Some(domain.to_string()),
            decl_span: None,
        };
        span_with_hover(&variable, ctx.source_code, ctx.source_map, hover);
    }

    Ok(vars)
}

#![allow(clippy::legacy_numeric_constants)]

use tree_sitter::Node;

use super::ParseContext;
use super::domain::parse_domain;
use crate::errors::FatalParseError;
use crate::field;
use conjure_cp_core::ast::DeclarationPtr;
use conjure_cp_core::ast::{Name, SymbolTable};

/// Parse a letting statement into a SymbolTable containing the declared symbols
pub fn parse_given(
    ctx: &mut ParseContext,
    given_statement: Node,
) -> Result<Option<SymbolTable>, FatalParseError> {
    let mut symbol_table = SymbolTable::new();

    let variable = field!(given_statement, "name");
    let domain = field!(given_statement, "domain");

    let variable_name = &ctx.source_code[variable.start_byte()..variable.end_byte()];
    let variable_name = Name::user(variable_name);
    let dom = parse_domain(ctx, domain)?.ok_or(FatalParseError::internal_error(
        "Expected domain".into(),
        Some(domain.range()),
    ))?;
    symbol_table.insert(DeclarationPtr::new_given(variable_name, dom));
    Ok(Some(symbol_table))
}

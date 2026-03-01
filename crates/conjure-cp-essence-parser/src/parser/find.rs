#![allow(clippy::legacy_numeric_constants)]

use std::collections::BTreeMap;
use tree_sitter::Node;

use super::domain::parse_domain;
use super::util::named_children;
use crate::diagnostics::diagnostics_api::SymbolKind;
use crate::diagnostics::source_map::{HoverInfo, SourceMap, span_with_hover};
use crate::errors::{FatalParseError, RecoverableParseError};
use crate::field;
use conjure_cp_core::ast::{DomainPtr, Name, SymbolTablePtr};

/// Parse a find statement into a map of decision variable names to their domains.
pub fn parse_find_statement(
    find_statement: Node,
    source_code: &str,
    symbols: Option<SymbolTablePtr>,
    errors: &mut Vec<RecoverableParseError>,
    source_map: &mut SourceMap,
) -> Result<BTreeMap<Name, DomainPtr>, FatalParseError> {
    let mut vars = BTreeMap::new();

    let domain = field!(find_statement, "domain");
    let Some(domain) = parse_domain(domain, source_code, symbols, errors, source_map)? else {
        return Ok(vars);
    };

    let variable_list = field!(find_statement, "variables");
    for variable in named_children(&variable_list) {
        let variable_name = &source_code[variable.start_byte()..variable.end_byte()];
        vars.insert(Name::user(variable_name), domain.clone());
        let hover = HoverInfo {
            description: format!("Find variable: {variable_name}"),
            kind: Some(SymbolKind::Find),
            ty: Some(domain.to_string()),
            decl_span: None,
        };
        span_with_hover(&variable, source_code, source_map, hover);
    }

    Ok(vars)
}

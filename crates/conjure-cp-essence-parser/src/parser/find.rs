#![allow(clippy::legacy_numeric_constants)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use tree_sitter::Node;

use super::domain::parse_domain;
use super::util::named_children;
use crate::{
    EssenceParseError,
    diagnostics::{
        diagnostics_api::SymbolKind,
        source_map::{HoverInfo, SourceMap, span_with_hover},
    },
};
use conjure_cp_core::ast::{DomainPtr, Name, SymbolTable};

/// Parse a find statement into a map of decision variable names to their domains.
pub fn parse_find_statement(
    find_statement: Node,
    source_code: &str,
    symbols: Option<Rc<RefCell<SymbolTable>>>,
    source_map: &mut SourceMap,
) -> Result<BTreeMap<Name, DomainPtr>, EssenceParseError> {
    let mut vars = BTreeMap::new();

    let domain = find_statement
        .child_by_field_name("domain")
        .expect("No domain found in find statement");
    let domain = parse_domain(domain, source_code, symbols, source_map)?;

    let variable_list = find_statement
        .child_by_field_name("variables")
        .expect("No variable list found");
    for variable in named_children(&variable_list) {
        let variable_name = &source_code[variable.start_byte()..variable.end_byte()];
        vars.insert(Name::user(variable_name), domain.clone());
    }

    let hover = HoverInfo {
        description: format!("Find variable(s) of type '{}'", domain),
        kind: Some(SymbolKind::Find),
        ty: Some(domain.to_string()),
        decl_span: None,
    };

    span_with_hover(&find_statement, source_code, source_map, hover);

    Ok(vars)
}

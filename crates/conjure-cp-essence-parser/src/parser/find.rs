#![allow(clippy::legacy_numeric_constants)]
use std::collections::BTreeMap;

use tree_sitter::Node;

use super::domain::parse_domain;
use super::util::named_children;
use crate::EssenceParseError;
use conjure_cp_core::ast::{Domain, Name};

/// Parse a find statement into a map of decision variable names to their domains.
pub fn parse_find_statement(
    find_statement: Node,
    source_code: &str,
) -> Result<BTreeMap<Name, Domain>, EssenceParseError> {
    let mut vars = BTreeMap::new();

    let domain = find_statement
        .child_by_field_name("domain")
        .expect("No domain found in find statement");
    let domain = parse_domain(domain, source_code)?;

    let variable_list = find_statement
        .child_by_field_name("variables")
        .expect("No variable list found");
    for variable in named_children(&variable_list) {
        let variable_name = &source_code[variable.start_byte()..variable.end_byte()];
        vars.insert(Name::user(variable_name), domain.clone());
    }

    Ok(vars)
}

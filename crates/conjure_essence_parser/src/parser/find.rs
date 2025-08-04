#![allow(clippy::legacy_numeric_constants)]
use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use conjure_core::ast::{Domain, Name};

use super::domain::parse_domain;
use super::util::named_children;

/// Parse a find statement into a map of decision variable names to their domains.
pub fn parse_find_statement(
    find_statement_list: Node,
    source_code: &str,
) -> BTreeMap<Name, Domain> {
    let mut vars = BTreeMap::new();

    for find_statement in named_children(&find_statement_list) {
        let mut temp_symbols = BTreeSet::new();

        let variable_list = find_statement
            .named_child(0)
            .expect("No variable list found");
        for variable in named_children(&variable_list) {
            let variable_name = &source_code[variable.start_byte()..variable.end_byte()];
            temp_symbols.insert(variable_name);
        }

        let domain = find_statement.named_child(1).expect("No domain found");
        let domain = parse_domain(domain, source_code);

        for name in temp_symbols {
            vars.insert(Name::user(name), domain.clone());
        }
    }
    vars
}

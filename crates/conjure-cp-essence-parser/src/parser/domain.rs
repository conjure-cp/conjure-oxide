use super::util::named_children;
use crate::EssenceParseError;
use crate::expression::parse_expression;
use conjure_cp_core::ast::{
    DeclarationPtr, Domain, DomainPtr, IntVal, Moo, Name, Range, RecordEntry, Reference, SetAttr,
    SymbolTablePtr,
};
use core::panic;
use std::str::FromStr;
use tree_sitter::Node;

/// Parse an Essence variable domain into its Conjure AST representation.
pub fn parse_domain(
    domain: Node,
    source_code: &str,
    symbols: Option<SymbolTablePtr>,
) -> Result<DomainPtr, EssenceParseError> {
    match domain.kind() {
        "domain" => parse_domain(
            domain.child(0).expect("No domain found"),
            source_code,
            symbols,
        ),
        "bool_domain" => Ok(Domain::bool()),
        "int_domain" => Ok(parse_int_domain(domain, source_code, &symbols)),
        "identifier" => {
            let decl = get_declaration_ptr_from_identifier(domain, source_code, &symbols)?;
            let dom = Domain::reference(decl).ok_or(EssenceParseError::syntax_error(
                format!(
                    "'{}' is not a valid domain declaration",
                    &source_code[domain.start_byte()..domain.end_byte()]
                ),
                Some(domain.range()),
            ))?;
            Ok(dom)
        }
        "tuple_domain" => parse_tuple_domain(domain, source_code, symbols),
        "matrix_domain" => parse_matrix_domain(domain, source_code, symbols),
        "record_domain" => parse_record_domain(domain, source_code, symbols),
        "set_domain" => parse_set_domain(domain, source_code, symbols),
        _ => panic!("{} is not a supported domain type", domain.kind()),
    }
}

fn get_declaration_ptr_from_identifier(
    identifier: Node,
    source_code: &str,
    symbols_ptr: &Option<SymbolTablePtr>,
) -> Result<DeclarationPtr, EssenceParseError> {
    let name = Name::user(&source_code[identifier.start_byte()..identifier.end_byte()]);
    let decl = symbols_ptr
        .as_ref()
        .ok_or(EssenceParseError::syntax_error(
            "context needed to resolve identifier".to_string(),
            Some(identifier.range()),
        ))?
        .read()
        .lookup(&name)
        .ok_or(EssenceParseError::syntax_error(
            format!("'{name}' is not defined"),
            Some(identifier.range()),
        ))?;
    Ok(decl)
}

/// Helper function to parse a node into an IntVal
/// Handles constants, references, and arbitrary expressions
fn parse_int_val(
    node: Node,
    source_code: &str,
    root: &Node,
    symbols_ptr: &Option<SymbolTablePtr>,
) -> Result<IntVal, EssenceParseError> {
    // For atoms, try to parse as a constant integer first
    if node.kind() == "atom" {
        let text = &source_code[node.start_byte()..node.end_byte()];
        if let Ok(integer) = text.parse::<i32>() {
            return Ok(IntVal::Const(integer));
        }
        // Otherwise, check if it's an identifier reference
        if let Ok(decl) = get_declaration_ptr_from_identifier(node, source_code, symbols_ptr) {
            return Ok(IntVal::Reference(Reference::new(decl)));
        }
    }

    // For anything else (arithmetic_expr, sum_expr, etc.), parse as an expression
    let expr = parse_expression(node, source_code, root, symbols_ptr.clone())?;
    Ok(IntVal::Expr(Moo::new(expr)))
}

/// Parse an integer domain. Can be a single integer or a range.
fn parse_int_domain(
    int_domain: Node,
    source_code: &str,
    symbols_ptr: &Option<SymbolTablePtr>,
) -> DomainPtr {
    if int_domain.child_count() == 1 {
        // for domains of just 'int' with no range
        return Domain::int(vec![Range::Bounded(i32::MIN, i32::MAX)]);
    }

    let range_list = int_domain
        .child_by_field_name("ranges")
        .expect("No range list found for int domain");

    let mut ranges_unresolved: Vec<Range<IntVal>> = Vec::new();
    let mut all_resolved = true;

    for domain_component in named_children(&range_list) {
        match domain_component.kind() {
            "atom" | "arithmetic_expr" => {
                let int_val =
                    parse_int_val(domain_component, source_code, &int_domain, symbols_ptr)
                        .unwrap_or_else(|_| panic!("Failed to parse integer value"));

                if !matches!(int_val, IntVal::Const(_)) {
                    all_resolved = false;
                }
                ranges_unresolved.push(Range::Single(int_val));
            }
            "int_range" => {
                let lower_bound = domain_component
                    .child_by_field_name("lower")
                    .map(|node| parse_int_val(node, source_code, &int_domain, symbols_ptr));
                let upper_bound = domain_component
                    .child_by_field_name("upper")
                    .map(|node| parse_int_val(node, source_code, &int_domain, symbols_ptr));

                match (lower_bound, upper_bound) {
                    (Some(Ok(lower)), Some(Ok(upper))) => {
                        if !matches!(lower, IntVal::Const(_)) || !matches!(upper, IntVal::Const(_))
                        {
                            all_resolved = false;
                        }
                        ranges_unresolved.push(Range::Bounded(lower, upper));
                    }
                    (Some(Ok(lower)), None) => {
                        if !matches!(lower, IntVal::Const(_)) {
                            all_resolved = false;
                        }
                        ranges_unresolved.push(Range::UnboundedR(lower));
                    }
                    (None, Some(Ok(upper))) => {
                        if !matches!(upper, IntVal::Const(_)) {
                            all_resolved = false;
                        }
                        ranges_unresolved.push(Range::UnboundedL(upper));
                    }
                    _ => panic!("Invalid int range"),
                }
            }
            _ => panic!("unsupported domain component: {}", domain_component.kind()),
        }
    }

    // If all values are resolved constants, convert IntVals to raw integers
    if all_resolved {
        let ranges: Vec<Range<i32>> = ranges_unresolved
            .into_iter()
            .map(|r| match r {
                Range::Single(IntVal::Const(v)) => Range::Single(v),
                Range::Bounded(IntVal::Const(l), IntVal::Const(u)) => Range::Bounded(l, u),
                Range::UnboundedR(IntVal::Const(l)) => Range::UnboundedR(l),
                Range::UnboundedL(IntVal::Const(u)) => Range::UnboundedL(u),
                Range::Unbounded => Range::Unbounded,
                _ => unreachable!("all_resolved should be true only if all are Const"),
            })
            .collect();
        Domain::int(ranges)
    } else {
        // Otherwise, keep as an expression-based domain
        Domain::int(ranges_unresolved)
    }
}

fn parse_tuple_domain(
    tuple_domain: Node,
    source_code: &str,
    symbols: Option<SymbolTablePtr>,
) -> Result<DomainPtr, EssenceParseError> {
    let mut domains: Vec<DomainPtr> = Vec::new();
    for domain in named_children(&tuple_domain) {
        domains.push(parse_domain(domain, source_code, symbols.clone())?);
    }
    Ok(Domain::tuple(domains))
}

fn parse_matrix_domain(
    matrix_domain: Node,
    source_code: &str,
    symbols: Option<SymbolTablePtr>,
) -> Result<DomainPtr, EssenceParseError> {
    let mut domains: Vec<DomainPtr> = Vec::new();
    let index_domain_list = matrix_domain
        .child_by_field_name("index_domain_list")
        .expect("No index domains found for matrix domain");
    for domain in named_children(&index_domain_list) {
        domains.push(parse_domain(domain, source_code, symbols.clone())?);
    }
    let value_domain = parse_domain(
        matrix_domain.child_by_field_name("value_domain").ok_or(
            EssenceParseError::syntax_error(
                "Expected a value domain".to_string(),
                Some(matrix_domain.range()),
            ),
        )?,
        source_code,
        symbols,
    )?;
    Ok(Domain::matrix(value_domain, domains))
}

fn parse_record_domain(
    record_domain: Node,
    source_code: &str,
    symbols: Option<SymbolTablePtr>,
) -> Result<DomainPtr, EssenceParseError> {
    let mut record_entries: Vec<RecordEntry> = Vec::new();
    for record_entry in named_children(&record_domain) {
        let name_node = record_entry
            .child_by_field_name("name")
            .expect("No name found for record entry");
        let name = Name::user(&source_code[name_node.start_byte()..name_node.end_byte()]);
        let domain_node = record_entry
            .child_by_field_name("domain")
            .expect("No domain found for record entry");
        let domain = parse_domain(domain_node, source_code, symbols.clone())?;
        record_entries.push(RecordEntry { name, domain });
    }
    Ok(Domain::record(record_entries))
}

pub fn parse_set_domain(
    set_domain: Node,
    source_code: &str,
    symbols: Option<SymbolTablePtr>,
) -> Result<DomainPtr, EssenceParseError> {
    let mut set_attribute: Option<SetAttr> = None;
    let mut value_domain: Option<DomainPtr> = None;

    for child in named_children(&set_domain) {
        match child.kind() {
            "set_attributes" => {
                // Check if we have both minSize and maxSize (minMax case)
                let min_value_node = child.child_by_field_name("min_value");
                let max_value_node = child.child_by_field_name("max_value");
                let size_value_node = child.child_by_field_name("size_value");

                if let (Some(min_node), Some(max_node)) = (min_value_node, max_value_node) {
                    // MinMax case
                    let min_str = &source_code[min_node.start_byte()..min_node.end_byte()];
                    let max_str = &source_code[max_node.start_byte()..max_node.end_byte()];

                    let min_val = i32::from_str(min_str).map_err(|_| {
                        EssenceParseError::syntax_error(
                            format!("Invalid integer value for minSize: {}", min_str),
                            Some(min_node.range()),
                        )
                    })?;

                    let max_val = i32::from_str(max_str).map_err(|_| {
                        EssenceParseError::syntax_error(
                            format!("Invalid integer value for maxSize: {}", max_str),
                            Some(max_node.range()),
                        )
                    })?;

                    set_attribute = Some(SetAttr::new_min_max_size(min_val, max_val));
                } else if let Some(size_node) = size_value_node {
                    // Size case
                    let size_str = &source_code[size_node.start_byte()..size_node.end_byte()];
                    let size_val = i32::from_str(size_str).map_err(|_| {
                        EssenceParseError::syntax_error(
                            format!("Invalid integer value for size: {}", size_str),
                            Some(size_node.range()),
                        )
                    })?;
                    set_attribute = Some(SetAttr::new_size(size_val));
                } else if let Some(min_node) = min_value_node {
                    // MinSize only case
                    let min_str = &source_code[min_node.start_byte()..min_node.end_byte()];
                    let min_val = i32::from_str(min_str).map_err(|_| {
                        EssenceParseError::syntax_error(
                            format!("Invalid integer value for minSize: {}", min_str),
                            Some(min_node.range()),
                        )
                    })?;
                    set_attribute = Some(SetAttr::new_min_size(min_val));
                } else if let Some(max_node) = max_value_node {
                    // MaxSize only case
                    let max_str = &source_code[max_node.start_byte()..max_node.end_byte()];
                    let max_val = i32::from_str(max_str).map_err(|_| {
                        EssenceParseError::syntax_error(
                            format!("Invalid integer value for maxSize: {}", max_str),
                            Some(max_node.range()),
                        )
                    })?;
                    set_attribute = Some(SetAttr::new_max_size(max_val));
                }
            }
            "domain" => {
                value_domain = Some(parse_domain(child, source_code, symbols.clone())?);
            }
            _ => {
                return Err(EssenceParseError::syntax_error(
                    format!("Unrecognized set domain child kind: {}", child.kind()),
                    Some(child.range()),
                ));
            }
        }
    }

    if let Some(domain) = value_domain {
        Ok(Domain::set(set_attribute.unwrap_or_default(), domain))
    } else {
        Err(EssenceParseError::syntax_error(
            "Set domain must have a value domain".to_string(),
            Some(set_domain.range()),
        ))
    }
}

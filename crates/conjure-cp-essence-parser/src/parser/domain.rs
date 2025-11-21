use super::util::named_children;
use crate::EssenceParseError;
use conjure_cp_core::ast::{Domain, DomainPtr, Name, Range, RecordEntry, SetAttr, SymbolTable};
use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;
use tree_sitter::Node;

/// Parse an Essence variable domain into its Conjure AST representation.
pub fn parse_domain(
    domain: Node,
    source_code: &str,
    symbols: Option<Rc<RefCell<SymbolTable>>>,
) -> Result<DomainPtr, EssenceParseError> {
    match domain.kind() {
        "domain" => parse_domain(
            domain.child(0).expect("No domain found"),
            source_code,
            symbols,
        ),
        "bool_domain" => Ok(Domain::new_bool()),
        "int_domain" => Ok(parse_int_domain(domain, source_code)),
        "identifier" => {
            let variable_name = &source_code[domain.start_byte()..domain.end_byte()];
            let name = Name::user(variable_name);
            let decl = symbols
                .ok_or(EssenceParseError::syntax_error(
                    "context needed to resolve domain lettings".to_string(),
                    Some(domain.range()),
                ))?
                .borrow()
                .lookup(&name)
                .ok_or(EssenceParseError::syntax_error(
                    format!("'{name}' is not defined"),
                    Some(domain.range()),
                ))?;
            let dom = Domain::new_ref(decl).ok_or(EssenceParseError::syntax_error(
                format!("'{}' is not a valid domain declaration", name),
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

/// Parse an integer domain. Can be a single integer or a range.
fn parse_int_domain(int_domain: Node, source_code: &str) -> DomainPtr {
    if int_domain.child_count() == 1 {
        Domain::new_int(vec![Range::Bounded(i32::MIN, i32::MAX)])
    } else {
        let mut ranges: Vec<Range<i32>> = Vec::new();
        let range_list = int_domain
            .child_by_field_name("ranges")
            .expect("No range list found (expression ranges not supported yet");
        for int_range in named_children(&range_list) {
            match int_range.kind() {
                "integer" => {
                    let integer_value = &source_code[int_range.start_byte()..int_range.end_byte()]
                        .parse::<i32>()
                        .unwrap();
                    ranges.push(Range::Single(*integer_value));
                }
                "int_range" => {
                    let lower_bound: Option<i32>;
                    let upper_bound: Option<i32>;
                    if let Some(lower_bound_node) = int_range.child_by_field_name("lower") {
                        lower_bound = Some(
                            source_code[lower_bound_node.start_byte()..lower_bound_node.end_byte()]
                                .parse::<i32>()
                                .unwrap(),
                        );
                    } else {
                        lower_bound = None;
                    }
                    if let Some(upper_bound_node) = int_range.child_by_field_name("upper") {
                        upper_bound = Some(
                            source_code[upper_bound_node.start_byte()..upper_bound_node.end_byte()]
                                .parse::<i32>()
                                .unwrap(),
                        );
                    } else {
                        upper_bound = None;
                    }

                    match (lower_bound, upper_bound) {
                        (Some(lb), Some(ub)) => ranges.push(Range::Bounded(lb, ub)),
                        (Some(lb), None) => ranges.push(Range::Bounded(lb, i32::MAX)),
                        (None, Some(ub)) => ranges.push(Range::Bounded(i32::MIN, ub)),
                        _ => panic!("Unsupported int range type"),
                    }
                }
                _ => panic!("unsupported int range type"),
            }
        }
        Domain::new_int(ranges)
    }
}

fn parse_tuple_domain(
    tuple_domain: Node,
    source_code: &str,
    symbols: Option<Rc<RefCell<SymbolTable>>>,
) -> Result<DomainPtr, EssenceParseError> {
    let mut domains: Vec<DomainPtr> = Vec::new();
    for domain in named_children(&tuple_domain) {
        domains.push(parse_domain(domain, source_code, symbols.clone())?);
    }
    Ok(Domain::new_tuple(domains))
}

fn parse_matrix_domain(
    matrix_domain: Node,
    source_code: &str,
    symbols: Option<Rc<RefCell<SymbolTable>>>,
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
    Ok(Domain::new_matrix(value_domain, domains))
}

fn parse_record_domain(
    record_domain: Node,
    source_code: &str,
    symbols: Option<Rc<RefCell<SymbolTable>>>,
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
    Ok(Domain::new_record(record_entries))
}

fn parse_set_domain(
    set_domain: Node,
    source_code: &str,
    symbols: Option<Rc<RefCell<SymbolTable>>>,
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
        Ok(Domain::new_set(set_attribute.unwrap_or_default(), domain))
    } else {
        Err(EssenceParseError::syntax_error(
            "Set domain must have a value domain".to_string(),
            Some(set_domain.range()),
        ))
    }
}

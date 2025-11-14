use super::util::named_children;
use crate::EssenceParseError;
use conjure_cp_core::ast::{
    Atom, Domain, Expression, Literal, Name, Range, RecordEntry, SetAttr, SymbolTable,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;
use tree_sitter::Node;

/// Parse an Essence variable domain into its Conjure AST representation.
pub fn parse_domain(
    domain: Node,
    source_code: &str,
    symbols_ptr: Option<Rc<RefCell<SymbolTable>>>,
) -> Result<Domain, EssenceParseError> {
    match domain.kind() {
        "domain" => parse_domain(
            domain.child(0).expect("No domain found"),
            source_code,
            symbols_ptr,
        ),
        "bool_domain" => Ok(Domain::Bool),
        "int_domain" => Ok(parse_int_domain(domain, source_code, symbols_ptr)),
        "identifier" => {
            let variable_name = &source_code[domain.start_byte()..domain.end_byte()];
            Ok(Domain::Reference(Name::user(variable_name)))
        }
        "tuple_domain" => parse_tuple_domain(domain, source_code, symbols_ptr),
        "matrix_domain" => parse_matrix_domain(domain, source_code, symbols_ptr),
        "record_domain" => parse_record_domain(domain, source_code, symbols_ptr),
        "set_domain" => parse_set_domain(domain, source_code, symbols_ptr),
        _ => panic!("{} is not a supported domain type", domain.kind()),
    }
}

/// Parse an integer domain. Can be a single integer or a range.
fn parse_int_domain(
    int_domain: Node,
    source_code: &str,
    symbols_ptr: Option<Rc<RefCell<SymbolTable>>>,
) -> Domain {
    if int_domain.child_count() == 1 {
        Domain::Int(vec![Range::Bounded(i32::MIN, i32::MAX)])
    } else {
        let mut ranges: Vec<Range<i32>> = Vec::new();
        let range_list = int_domain
            .child_by_field_name("ranges")
            .expect("No range list found (expression ranges not supported yet");
        for domain_component in named_children(&range_list) {
            match domain_component.kind() {
                "arithmetic_expr" => {
                    let text =
                        &source_code[domain_component.start_byte()..domain_component.end_byte()];
                    let value = parse_int_value(text, &symbols_ptr, "Domain value");
                    ranges.push(Range::Single(value));
                }
                "int_range" => {
                    let lower_bound = domain_component.child_by_field_name("lower").map(|node| {
                        let text = &source_code[node.start_byte()..node.end_byte()];
                        parse_int_value(text, &symbols_ptr, "Lower bound")
                    });

                    let upper_bound = domain_component.child_by_field_name("upper").map(|node| {
                        let text = &source_code[node.start_byte()..node.end_byte()];
                        parse_int_value(text, &symbols_ptr, "Upper bound")
                    });

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
        Domain::Int(ranges)
    }
}

fn parse_int_value(
    text: &str,
    symbols_ptr: &Option<Rc<RefCell<SymbolTable>>>,
    context: &str,
) -> i32 {
    // Try parsing as a literal integer first
    if let Ok(value) = text.parse::<i32>() {
        return value;
    }

    // Otherwise, look up the identifier in the symbol table
    if let Some(symbols) = symbols_ptr {
        let symbols = symbols.borrow();
        let name = Name::user(text);
        if let Some(decl_ptr) = symbols.lookup(&name) {
            if let Some(expr_ref) = decl_ptr.as_value_letting() {
                if let Expression::Atomic(_, Atom::Literal(Literal::Int(i))) = &*expr_ref {
                    return *i;
                } else {
                    panic!(
                        "{} identifier '{}' is not an integer literal",
                        context, text
                    );
                }
            } else {
                panic!("{} identifier '{}' is not a value letting", context, text);
            }
        } else {
            panic!(
                "{} identifier '{}' not found in symbol table",
                context, text
            );
        }
    } else {
        panic!(
            "{} identifier '{}' used but no symbol table provided",
            context, text
        );
    }
}

fn parse_tuple_domain(
    tuple_domain: Node,
    source_code: &str,
    symbols_ptr: Option<Rc<RefCell<SymbolTable>>>,
) -> Result<Domain, EssenceParseError> {
    let mut domains: Vec<Domain> = Vec::new();
    for domain in named_children(&tuple_domain) {
        domains.push(parse_domain(domain, source_code, symbols_ptr.clone())?);
    }
    Ok(Domain::Tuple(domains))
}

fn parse_matrix_domain(
    matrix_domain: Node,
    source_code: &str,
    symbols_ptr: Option<Rc<RefCell<SymbolTable>>>,
) -> Result<Domain, EssenceParseError> {
    let mut domains: Vec<Domain> = Vec::new();
    let index_domain_list = matrix_domain
        .child_by_field_name("index_domain_list")
        .expect("No index domains found for matrix domain");
    for domain in named_children(&index_domain_list) {
        domains.push(parse_domain(domain, source_code, symbols_ptr.clone())?);
    }
    let value_domain = parse_domain(
        matrix_domain.child_by_field_name("value_domain").ok_or(
            EssenceParseError::syntax_error(
                "Expected a value domain".to_string(),
                Some(matrix_domain.range()),
            ),
        )?,
        source_code,
        symbols_ptr,
    )?;
    Ok(Domain::Matrix(Box::new(value_domain), domains))
}

fn parse_record_domain(
    record_domain: Node,
    source_code: &str,
    symbols_ptr: Option<Rc<RefCell<SymbolTable>>>,
) -> Result<Domain, EssenceParseError> {
    let mut record_entries: Vec<RecordEntry> = Vec::new();
    for record_entry in named_children(&record_domain) {
        let name_node = record_entry
            .child_by_field_name("name")
            .expect("No name found for record entry");
        let name = Name::user(&source_code[name_node.start_byte()..name_node.end_byte()]);
        let domain_node = record_entry
            .child_by_field_name("domain")
            .expect("No domain found for record entry");
        let domain = parse_domain(domain_node, source_code, symbols_ptr.clone())?;
        record_entries.push(RecordEntry { name, domain });
    }
    Ok(Domain::Record(record_entries))
}

fn parse_set_domain(
    set_domain: Node,
    source_code: &str,
    symbols_ptr: Option<Rc<RefCell<SymbolTable>>>,
) -> Result<Domain, EssenceParseError> {
    let mut set_attribute: Option<SetAttr> = None;
    let mut value_domain: Option<Domain> = None;

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

                    set_attribute = Some(SetAttr::MinMaxSize(min_val, max_val));
                } else if let Some(size_node) = size_value_node {
                    // Size case
                    let size_str = &source_code[size_node.start_byte()..size_node.end_byte()];
                    let size_val = i32::from_str(size_str).map_err(|_| {
                        EssenceParseError::syntax_error(
                            format!("Invalid integer value for size: {}", size_str),
                            Some(size_node.range()),
                        )
                    })?;
                    set_attribute = Some(SetAttr::Size(size_val));
                } else if let Some(min_node) = min_value_node {
                    // MinSize only case
                    let min_str = &source_code[min_node.start_byte()..min_node.end_byte()];
                    let min_val = i32::from_str(min_str).map_err(|_| {
                        EssenceParseError::syntax_error(
                            format!("Invalid integer value for minSize: {}", min_str),
                            Some(min_node.range()),
                        )
                    })?;
                    set_attribute = Some(SetAttr::MinSize(min_val));
                } else if let Some(max_node) = max_value_node {
                    // MaxSize only case
                    let max_str = &source_code[max_node.start_byte()..max_node.end_byte()];
                    let max_val = i32::from_str(max_str).map_err(|_| {
                        EssenceParseError::syntax_error(
                            format!("Invalid integer value for maxSize: {}", max_str),
                            Some(max_node.range()),
                        )
                    })?;
                    set_attribute = Some(SetAttr::MaxSize(max_val));
                }
            }
            "domain" => {
                value_domain = Some(parse_domain(child, source_code, symbols_ptr.clone())?);
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
        Ok(Domain::Set(
            set_attribute.unwrap_or(SetAttr::None),
            Box::new(domain),
        ))
    } else {
        Err(EssenceParseError::syntax_error(
            "Set domain must have a value domain".to_string(),
            Some(set_domain.range()),
        ))
    }
}

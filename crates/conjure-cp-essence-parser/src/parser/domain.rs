use super::atom::parse_int;
use super::util::named_children;
use crate::errors::{FatalParseError, RecoverableParseError};
use crate::{child, field};
use conjure_cp_core::ast::{
    DeclarationPtr, Domain, DomainPtr, IntVal, Moo, Name, Range, RecordEntry, Reference, SetAttr,
    SymbolTablePtr,
};
use crate::expression::parse_expression;
use tree_sitter::Node;

/// Parse an Essence variable domain into its Conjure AST representation.
pub fn parse_domain(
    domain: Node,
    source_code: &str,
    symbols: Option<SymbolTablePtr>,
    errors: &mut Vec<RecoverableParseError>,
) -> Result<Option<DomainPtr>, FatalParseError> {
    match domain.kind() {
        "domain" => parse_domain(child!(domain, 0, "domain"), source_code, symbols, errors),
        "bool_domain" => Ok(Some(Domain::bool())),
        "int_domain" => parse_int_domain(domain, source_code, &symbols, errors),
        "identifier" => {
            let Some(decl) =
                get_declaration_ptr_from_identifier(domain, source_code, &symbols, errors)?
            else {
                return Ok(None);
            };
            let Some(dom) = Domain::reference(decl) else {
                errors.push(RecoverableParseError::new(
                    format!(
                        "The identifier '{}' is not a valid domain",
                        &source_code[domain.start_byte()..domain.end_byte()]
                    ),
                    Some(domain.range()),
                ));
                return Ok(None);
            };
            Ok(Some(dom))
        }
        "tuple_domain" => parse_tuple_domain(domain, source_code, symbols, errors),
        "matrix_domain" => parse_matrix_domain(domain, source_code, symbols, errors),
        "record_domain" => parse_record_domain(domain, source_code, symbols, errors),
        "set_domain" => parse_set_domain(domain, source_code, symbols, errors),
        _ => return Err(FatalParseError::internal_error(
            format!("Unexpected domain type: {}", domain.kind()),
            Some(domain.range()),
        )),
    }
}

fn get_declaration_ptr_from_identifier(
    identifier: Node,
    source_code: &str,
    symbols_ptr: &Option<SymbolTablePtr>,
    errors: &mut Vec<RecoverableParseError>,
) -> Result<Option<DeclarationPtr>, FatalParseError> {
    let name = Name::user(&source_code[identifier.start_byte()..identifier.end_byte()]);
    let decl = symbols_ptr
        .as_ref()
        .ok_or(FatalParseError::internal_error(
            "context needed to resolve identifier".to_string(),
            Some(identifier.range()),
        ))?
        .read()
        .lookup(&name);
    match decl {
        Some(decl) => Ok(Some(decl)),
        None => {
            errors.push(RecoverableParseError::new(
                format!("The identifier '{}' is not defined", name),
                Some(identifier.range()),
            ));
            Ok(None)
        }
    }
}

/// Parse an integer domain. Can be a single integer or a range.
fn parse_int_domain(
    int_domain: Node,
    source_code: &str,
    symbols_ptr: &Option<SymbolTablePtr>,
    errors: &mut Vec<RecoverableParseError>,
) -> Result<Option<DomainPtr>, FatalParseError> {
    if int_domain.child_count() == 1 {
        // for domains of just 'int' with no range
        return Ok(Some(Domain::int(vec![Range::Bounded(i32::MIN, i32::MAX)])));
    }

    let range_list = field!(int_domain, "ranges");
    let mut ranges_unresolved: Vec<Range<IntVal>> = Vec::new();
    let mut all_resolved = true;

    for domain_component in named_children(&range_list) {
        match domain_component.kind() {
            "atom" | "arithmetic_expr" => {
                let Some(int_val) = parse_int_val(domain_component, source_code, symbols_ptr, errors)? else {
                    return Ok(None);
                };

                if !matches!(int_val, IntVal::Const(_)) {
                    all_resolved = false;
                }
                ranges_unresolved.push(Range::Single(int_val));
            }
            "int_range" => {
                let lower_bound = match domain_component.child_by_field_name("lower") {
                    Some(node) => match parse_int_val(node, source_code, symbols_ptr, errors)? {
                        Some(val) => Some(val),
                        None => return Ok(None), // semantic error occurred
                    },
                    None => None,
                };
                let upper_bound = match domain_component.child_by_field_name("upper") {
                    Some(node) => match parse_int_val(node, source_code, symbols_ptr, errors)? {
                        Some(val) => Some(val),
                        None => return Ok(None), // semantic error occurred
                    },
                    None => None,
                };

                match (lower_bound, upper_bound) {
                    (Some(lower), Some(upper)) => {
                        if !matches!(lower, IntVal::Const(_)) || !matches!(upper, IntVal::Const(_))
                        {
                            all_resolved = false;
                        }
                        ranges_unresolved.push(Range::Bounded(lower, upper));
                    }
                    (Some(lower), None) => {
                        if !matches!(lower, IntVal::Const(_)) {
                            all_resolved = false;
                        }
                        ranges_unresolved.push(Range::UnboundedR(lower));
                    }
                    (None, Some(upper)) => {
                        if !matches!(upper, IntVal::Const(_)) {
                            all_resolved = false;
                        }
                        ranges_unresolved.push(Range::UnboundedL(upper));
                    }
                    _ => return Err(FatalParseError::internal_error(
                        "Invalid int range: must have at least a lower or upper bound".to_string(),
                        Some(domain_component.range()),
                    )),
                }
            }
            _ => return Err(FatalParseError::internal_error(
                format!(
                    "Unexpected int domain component: {}",
                    domain_component.kind()
                ),
                Some(domain_component.range()),
            )),
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
        Ok(Some(Domain::int(ranges)))
    } else {
        // Otherwise, keep as an expression-based domain
        Ok(Some(Domain::int(ranges_unresolved)))
    }
}

// Helper function to parse a node into an IntVal
// Handles constants, references, and arbitrary expressions
fn parse_int_val(
    node: Node,
    source_code: &str,
    symbols_ptr: &Option<SymbolTablePtr>,
    errors: &mut Vec<RecoverableParseError>,
) -> Result<Option<IntVal>, FatalParseError> {
    // For atoms, try to parse as a constant integer first
    if node.kind() == "atom" {
        let text = &source_code[node.start_byte()..node.end_byte()];
        if let Ok(integer) = text.parse::<i32>() {
            return Ok(Some(IntVal::Const(integer)));
        }
        // Otherwise, check if it's an identifier reference
        let Some(decl) = get_declaration_ptr_from_identifier(node, source_code, symbols_ptr, errors)? else {
            // If identifier isn't defined, its a semantic error
            return Ok(None);
        };
        return Ok(Some(IntVal::Reference(Reference::new(decl))));
    }

    // For anything else, parse as an expression
    let Some(expr) = parse_expression(node, source_code, &node, symbols_ptr.clone(), errors)? else {
        return Ok(None);
    };
    Ok(Some(IntVal::Expr(Moo::new(expr))))
}


fn parse_tuple_domain(
    tuple_domain: Node,
    source_code: &str,
    symbols: Option<SymbolTablePtr>,
    errors: &mut Vec<RecoverableParseError>,
) -> Result<Option<DomainPtr>, FatalParseError> {
    let mut domains: Vec<DomainPtr> = Vec::new();
    for domain in named_children(&tuple_domain) {
        let Some(parsed_domain) = parse_domain(domain, source_code, symbols.clone(), errors)?
        else {
            return Ok(None);
        };
        domains.push(parsed_domain);
    }
    Ok(Some(Domain::tuple(domains)))
}

fn parse_matrix_domain(
    matrix_domain: Node,
    source_code: &str,
    symbols: Option<SymbolTablePtr>,
    errors: &mut Vec<RecoverableParseError>,
) -> Result<Option<DomainPtr>, FatalParseError> {
    let mut domains: Vec<DomainPtr> = Vec::new();
    let index_domain_list = field!(matrix_domain, "index_domain_list");
    for domain in named_children(&index_domain_list) {
        let Some(parsed_domain) = parse_domain(domain, source_code, symbols.clone(), errors)?
        else {
            return Ok(None);
        };
        domains.push(parsed_domain);
    }
    let Some(value_domain) = parse_domain(
        field!(matrix_domain, "value_domain"),
        source_code,
        symbols,
        errors,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(Domain::matrix(value_domain, domains)))
}

fn parse_record_domain(
    record_domain: Node,
    source_code: &str,
    symbols: Option<SymbolTablePtr>,
    errors: &mut Vec<RecoverableParseError>,
) -> Result<Option<DomainPtr>, FatalParseError> {
    let mut record_entries: Vec<RecordEntry> = Vec::new();
    for record_entry in named_children(&record_domain) {
        let name_node = field!(record_entry, "name");
        let name = Name::user(&source_code[name_node.start_byte()..name_node.end_byte()]);
        let domain_node = field!(record_entry, "domain");
        let Some(domain) = parse_domain(domain_node, source_code, symbols.clone(), errors)? else {
            return Ok(None);
        };
        record_entries.push(RecordEntry { name, domain });
    }
    Ok(Some(Domain::record(record_entries)))
}

pub fn parse_set_domain(
    set_domain: Node,
    source_code: &str,
    symbols: Option<SymbolTablePtr>,
    errors: &mut Vec<RecoverableParseError>,
) -> Result<Option<DomainPtr>, FatalParseError> {
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
                    let min_val = parse_int(&min_node, source_code, errors)?;
                    let max_val = parse_int(&max_node, source_code, errors)?;

                    set_attribute = Some(SetAttr::new_min_max_size(min_val, max_val));
                } else if let Some(size_node) = size_value_node {
                    // Size case
                    let size_val = parse_int(&size_node, source_code, errors)?;
                    set_attribute = Some(SetAttr::new_size(size_val));
                } else if let Some(min_node) = min_value_node {
                    // MinSize only case
                    let min_val = parse_int(&min_node, source_code, errors)?;
                    set_attribute = Some(SetAttr::new_min_size(min_val));
                } else if let Some(max_node) = max_value_node {
                    // MaxSize only case
                    let max_val = parse_int(&max_node, source_code, errors)?;
                    set_attribute = Some(SetAttr::new_max_size(max_val));
                }
            }
            "domain" => {
                let Some(parsed_domain) =
                    parse_domain(child, source_code, symbols.clone(), errors)?
                else {
                    return Ok(None);
                };
                value_domain = Some(parsed_domain);
            }
            _ => {
                return Err(FatalParseError::internal_error(
                    format!("Unrecognized set domain child kind: {}", child.kind()),
                    Some(child.range()),
                ));
            }
        }
    }

    if let Some(domain) = value_domain {
        Ok(Some(Domain::set(set_attribute.unwrap_or_default(), domain)))
    } else {
        Err(FatalParseError::internal_error(
            "Set domain must have a value domain".to_string(),
            Some(set_domain.range()),
        ))
    }
}

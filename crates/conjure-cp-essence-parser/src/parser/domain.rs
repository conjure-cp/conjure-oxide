use super::atom::parse_int;
use super::util::named_children;
use crate::RecoverableParseError;
use crate::diagnostics::source_map::{HoverInfo, span_with_hover};
use crate::errors::FatalParseError;
use crate::expression::parse_expression;
use crate::parser::ParseContext;
use conjure_cp_core::ast::{
    DeclarationPtr, Domain, DomainPtr, IntVal, Moo, Name, Range, RecordEntry, Reference, SetAttr,
};
use tree_sitter::Node;

use crate::field;

/// Parse an Essence variable domain into its Conjure AST representation.
pub fn parse_domain(
    ctx: &mut ParseContext,
    domain: Node,
) -> Result<Option<DomainPtr>, FatalParseError> {
    match domain.kind() {
        "domain" => {
            let inner = match domain.child(0) {
                Some(node) => node,
                None => {
                    ctx.record_error(RecoverableParseError::new(
                        format!("{} in expression of kind '{}'", "domain", domain.kind()),
                        Some(domain.range()),
                    ));
                    return Ok(None);
                }
            };
            parse_domain(ctx, inner)
        }
        "bool_domain" => {
            let hover = HoverInfo {
                description: "Boolean domain".to_string(),
                kind: Some(crate::diagnostics::diagnostics_api::SymbolKind::Domain),
                ty: None,
                decl_span: None,
            };
            span_with_hover(&domain, ctx.source_code, ctx.source_map, hover);
            Ok(Some(Domain::bool()))
        }
        "int_domain" => {
            let hover = HoverInfo {
                description: "Integer domain".to_string(),
                kind: Some(crate::diagnostics::diagnostics_api::SymbolKind::Domain),
                ty: None,
                decl_span: None,
            };
            span_with_hover(&domain, ctx.source_code, ctx.source_map, hover);
            parse_int_domain(ctx, domain)
        }
        "identifier" => {
            let Some(decl) = get_declaration_ptr_from_identifier(ctx, domain)? else {
                return Ok(None);
            };
            let Some(dom) = Domain::reference(decl) else {
                ctx.record_error(crate::errors::RecoverableParseError::new(
                    format!(
                        "The identifier '{}' is not a valid domain",
                        &ctx.source_code[domain.start_byte()..domain.end_byte()]
                    ),
                    Some(domain.range()),
                ));
                return Ok(None);
            };
            let name = &ctx.source_code[domain.start_byte()..domain.end_byte()];
            let hover = HoverInfo {
                description: format!("Domain reference: {name}"),
                kind: None,
                ty: None,
                decl_span: None,
            };
            span_with_hover(&domain, ctx.source_code, ctx.source_map, hover);
            Ok(Some(dom))
        }
        "tuple_domain" => parse_tuple_domain(ctx, domain),
        "matrix_domain" => parse_matrix_domain(ctx, domain),
        "record_domain" => parse_record_domain(ctx, domain),
        "set_domain" => parse_set_domain(ctx, domain),
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("{} is not a supported domain type", domain.kind()),
                Some(domain.range()),
            ));
            Ok(None)
        }
    }
}

fn get_declaration_ptr_from_identifier(
    ctx: &mut ParseContext,
    identifier: Node,
) -> Result<Option<DeclarationPtr>, FatalParseError> {
    let name = Name::user(&ctx.source_code[identifier.start_byte()..identifier.end_byte()]);
    let decl = ctx.symbols.as_ref().unwrap().read().lookup(&name);

    if decl.is_none() {
        ctx.record_error(crate::errors::RecoverableParseError::new(
            format!("The identifier '{}' is not defined", name),
            Some(identifier.range()),
        ));
        return Ok(None);
    }
    match decl {
        Some(decl) => Ok(Some(decl)),
        None => {
            ctx.record_error(crate::errors::RecoverableParseError::new(
                format!("The identifier '{}' is not defined", name),
                Some(identifier.range()),
            ));
            Ok(None)
        }
    }
}

/// Parse an integer domain. Can be a single integer or a range.
fn parse_int_domain(
    ctx: &mut ParseContext,
    int_domain: Node,
) -> Result<Option<DomainPtr>, FatalParseError> {
    if int_domain.child_count() == 1 {
        // for domains of just 'int' with no range
        return Ok(Some(Domain::int(vec![Range::Bounded(i32::MIN, i32::MAX)])));
    }

    let Some(range_list) = field!(recover, ctx, int_domain, "ranges") else {
        return Ok(None);
    };
    let mut ranges_unresolved: Vec<Range<IntVal>> = Vec::new();
    let mut all_resolved = true;

    for domain_component in named_children(&range_list) {
        match domain_component.kind() {
            "atom" | "arithmetic_expr" => {
                let Some(int_val) = parse_int_val(ctx, domain_component)? else {
                    return Ok(None);
                };

                if !matches!(int_val, IntVal::Const(_)) {
                    all_resolved = false;
                }
                ranges_unresolved.push(Range::Single(int_val));
            }
            "int_range" => {
                let lower_bound = match domain_component.child_by_field_name("lower") {
                    Some(node) => {
                        match parse_int_val(ctx, node)? {
                            Some(val) => Some(val),
                            None => return Ok(None), // semantic error occurred
                        }
                    }
                    None => None,
                };
                let upper_bound = match domain_component.child_by_field_name("upper") {
                    Some(node) => {
                        match parse_int_val(ctx, node)? {
                            Some(val) => Some(val),
                            None => return Ok(None), // semantic error occurred
                        }
                    }
                    None => None,
                };

                match (lower_bound, upper_bound) {
                    (Some(lower), Some(upper)) => {
                        // Check if both bounds are constants and validate lower <= upper
                        if let (IntVal::Const(l), IntVal::Const(u)) = (&lower, &upper) {
                            if l > u {
                                ctx.record_error(crate::errors::RecoverableParseError::new(
                                    format!(
                                        "Invalid integer range: lower bound {} is greater than upper bound {}",
                                        l, u
                                    ),
                                    Some(domain_component.range()),
                                ));
                            }
                        } else {
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
                    _ => {
                        ctx.record_error(RecoverableParseError::new(
                            "Invalid int range: must have at least a lower or upper bound"
                                .to_string(),
                            Some(domain_component.range()),
                        ));
                        return Ok(None);
                    }
                }
            }
            _ => {
                ctx.record_error(RecoverableParseError::new(
                    format!(
                        "Unexpected int domain component: {}",
                        domain_component.kind()
                    ),
                    Some(domain_component.range()),
                ));
                return Ok(None);
            }
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
fn parse_int_val(ctx: &mut ParseContext, node: Node) -> Result<Option<IntVal>, FatalParseError> {
    // For atoms, try to parse as a constant integer first
    if node.kind() == "atom" {
        let text = &ctx.source_code[node.start_byte()..node.end_byte()];
        if let Ok(integer) = text.parse::<i32>() {
            return Ok(Some(IntVal::Const(integer)));
        }
        // Otherwise, check if it's an identifier reference
        let Some(decl) = get_declaration_ptr_from_identifier(ctx, node)? else {
            // If identifier isn't defined, its a semantic error
            return Ok(None);
        };
        return Ok(Some(IntVal::Reference(Reference::new(decl))));
    }

    // For anything else, parse as an expression
    let Some(expr) = parse_expression(ctx, node)? else {
        return Ok(None);
    };
    Ok(Some(IntVal::Expr(Moo::new(expr))))
}

fn parse_tuple_domain(
    ctx: &mut ParseContext,
    tuple_domain: Node,
) -> Result<Option<DomainPtr>, FatalParseError> {
    let mut domains: Vec<DomainPtr> = Vec::new();
    for domain in named_children(&tuple_domain) {
        let Some(parsed_domain) = parse_domain(ctx, domain)? else {
            return Ok(None);
        };
        domains.push(parsed_domain);
    }
    Ok(Some(Domain::tuple(domains)))
}

fn parse_matrix_domain(
    ctx: &mut ParseContext,
    matrix_domain: Node,
) -> Result<Option<DomainPtr>, FatalParseError> {
    let mut domains: Vec<DomainPtr> = Vec::new();
    let Some(index_domain_list) = field!(recover, ctx, matrix_domain, "index_domain_list") else {
        return Ok(None);
    };
    for domain in named_children(&index_domain_list) {
        let Some(parsed_domain) = parse_domain(ctx, domain)? else {
            return Ok(None);
        };
        domains.push(parsed_domain);
    }
    let Some(value_domain_node) = field!(recover, ctx, matrix_domain, "value_domain") else {
        return Ok(None);
    };
    let Some(value_domain) = parse_domain(ctx, value_domain_node)? else {
        return Ok(None);
    };
    Ok(Some(Domain::matrix(value_domain, domains)))
}

fn parse_record_domain(
    ctx: &mut ParseContext,
    record_domain: Node,
) -> Result<Option<DomainPtr>, FatalParseError> {
    let mut record_entries: Vec<RecordEntry> = Vec::new();
    for record_entry in named_children(&record_domain) {
        let Some(name_node) = field!(recover, ctx, record_entry, "name") else {
            return Ok(None);
        };
        let name = Name::user(&ctx.source_code[name_node.start_byte()..name_node.end_byte()]);
        let Some(domain_node) = field!(recover, ctx, record_entry, "domain") else {
            return Ok(None);
        };
        let Some(domain) = parse_domain(ctx, domain_node)? else {
            return Ok(None);
        };
        record_entries.push(RecordEntry { name, domain });
    }
    Ok(Some(Domain::record(record_entries)))
}

pub fn parse_set_domain(
    ctx: &mut ParseContext,
    set_domain: Node,
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
                    let Some(min_val) = parse_int(ctx, &min_node) else {
                        return Ok(None);
                    };
                    let Some(max_val) = parse_int(ctx, &max_node) else {
                        return Ok(None);
                    };

                    set_attribute = Some(SetAttr::new_min_max_size(min_val, max_val));
                } else if let Some(size_node) = size_value_node {
                    // Size case
                    let Some(size_val) = parse_int(ctx, &size_node) else {
                        return Ok(None);
                    };
                    set_attribute = Some(SetAttr::new_size(size_val));
                } else if let Some(min_node) = min_value_node {
                    // MinSize only case
                    let Some(min_val) = parse_int(ctx, &min_node) else {
                        return Ok(None);
                    };
                    set_attribute = Some(SetAttr::new_min_size(min_val));
                } else if let Some(max_node) = max_value_node {
                    // MaxSize only case
                    let Some(max_val) = parse_int(ctx, &max_node) else {
                        return Ok(None);
                    };
                    set_attribute = Some(SetAttr::new_max_size(max_val));
                }
            }
            "domain" => {
                let Some(parsed_domain) = parse_domain(ctx, child)? else {
                    return Ok(None);
                };
                value_domain = Some(parsed_domain);
            }
            _ => {
                ctx.record_error(RecoverableParseError::new(
                    format!("Unrecognized set domain child kind: {}", child.kind()),
                    Some(child.range()),
                ));
                return Ok(None);
            }
        }
    }

    if let Some(domain) = value_domain {
        Ok(Some(Domain::set(set_attribute.unwrap_or_default(), domain)))
    } else {
        ctx.record_error(RecoverableParseError::new(
            "Set domain must have a value domain".to_string(),
            Some(set_domain.range()),
        ));
        Ok(None)
    }
}

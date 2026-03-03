use super::atom::parse_int;
use super::util::named_children;
use crate::diagnostics::source_map::{HoverInfo, SourceMap, span_with_hover};
use crate::errors::FatalParseError;
use crate::parser::ParseContext;
use crate::{child, field};
use conjure_cp_core::ast::{
    DeclarationPtr, Domain, DomainPtr, IntVal, Name, Range, RecordEntry, Reference, SetAttr,
};
use tree_sitter::Node;

/// Parse an Essence variable domain into its Conjure AST representation.
pub fn parse_domain(
    ctx: &mut ParseContext,
    domain: Node,
) -> Result<Option<DomainPtr>, FatalParseError> {
    match domain.kind() {
        "domain" => parse_domain(ctx, child!(domain, 0, "domain")),
        "bool_domain" => Ok(Some(Domain::bool())),
        "int_domain" => parse_int_domain(ctx, domain),
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
        _ => Err(FatalParseError::internal_error(
            format!("{} is not a supported domain type", domain.kind()),
            Some(domain.range()),
        )),
    }
}

fn get_declaration_ptr_from_identifier(
    ctx: &mut ParseContext,
    identifier: Node,
) -> Result<Option<DeclarationPtr>, FatalParseError> {
    let name = Name::user(&ctx.source_code[identifier.start_byte()..identifier.end_byte()]);
    let decl = ctx
        .symbols
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
        return Ok(Some(Domain::int(vec![Range::Bounded(i32::MIN, i32::MAX)])));
    }
    let mut ranges: Vec<Range<i32>> = Vec::new();
    let mut ranges_unresolved: Vec<Range<IntVal>> = Vec::new();
    let range_list = field!(int_domain, "ranges");
    for domain_component in named_children(&range_list) {
        match domain_component.kind() {
            "atom" => {
                let text =
                    &ctx.source_code[domain_component.start_byte()..domain_component.end_byte()];
                // Try parsing as a literal integer first
                if let Ok(integer) = text.parse::<i32>() {
                    ranges.push(Range::Single(integer));
                    continue;
                }
                // Otherwise, treat as a reference
                let Some(decl) = get_declaration_ptr_from_identifier(ctx, domain_component)? else {
                    return Ok(None);
                };
                ranges_unresolved.push(Range::Single(IntVal::Reference(Reference::new(decl))));
            }
            "int_range" => {
                let lower_bound: Option<Result<i32, DeclarationPtr>> = match domain_component
                    .child_by_field_name("lower")
                {
                    Some(lower_node) => {
                        // Try parsing as a literal integer first
                        let text = &ctx.source_code[lower_node.start_byte()..lower_node.end_byte()];
                        if let Ok(integer) = text.parse::<i32>() {
                            Some(Ok(integer))
                        } else {
                            let Some(decl) = get_declaration_ptr_from_identifier(ctx, lower_node)?
                            else {
                                return Ok(None); // return from function if we can't resolve the identifier
                            };
                            Some(Err(decl))
                        }
                    }
                    None => None,
                };
                let upper_bound: Option<Result<i32, DeclarationPtr>> = match domain_component
                    .child_by_field_name("upper")
                {
                    Some(upper_node) => {
                        // Try parsing as a literal integer first
                        let text = &ctx.source_code[upper_node.start_byte()..upper_node.end_byte()];
                        if let Ok(integer) = text.parse::<i32>() {
                            Some(Ok(integer))
                        } else {
                            let Some(decl) = get_declaration_ptr_from_identifier(ctx, upper_node)?
                            else {
                                return Ok(None); // return from function if we can't resolve the identifier
                            };
                            Some(Err(decl))
                        }
                    }
                    None => None,
                };

                match (lower_bound, upper_bound) {
                    (Some(Ok(lower)), Some(Ok(upper))) => ranges.push(Range::Bounded(lower, upper)),
                    (Some(Ok(lower)), Some(Err(decl))) => {
                        ranges_unresolved.push(Range::Bounded(
                            IntVal::Const(lower),
                            IntVal::Reference(Reference::new(decl)),
                        ));
                    }
                    (Some(Err(decl)), Some(Ok(upper))) => {
                        ranges_unresolved.push(Range::Bounded(
                            IntVal::Reference(Reference::new(decl)),
                            IntVal::Const(upper),
                        ));
                    }
                    (Some(Err(decl_lower)), Some(Err(decl_upper))) => {
                        ranges_unresolved.push(Range::Bounded(
                            IntVal::Reference(Reference::new(decl_lower)),
                            IntVal::Reference(Reference::new(decl_upper)),
                        ));
                    }
                    (Some(Ok(lower)), None) => {
                        ranges.push(Range::UnboundedR(lower));
                    }
                    (Some(Err(decl)), None) => {
                        ranges_unresolved
                            .push(Range::UnboundedR(IntVal::Reference(Reference::new(decl))));
                    }
                    (None, Some(Ok(upper))) => {
                        ranges.push(Range::UnboundedL(upper));
                    }
                    (None, Some(Err(decl))) => {
                        ranges_unresolved
                            .push(Range::UnboundedL(IntVal::Reference(Reference::new(decl))));
                    }
                    (None, None) => {
                        ranges.push(Range::Unbounded);
                    }
                }
            }
            _ => panic!("unsupported int range type"),
        }
    }

    if !ranges_unresolved.is_empty() {
        for range in ranges {
            match range {
                Range::Single(i) => ranges_unresolved.push(Range::Single(IntVal::Const(i))),
                Range::Bounded(l, u) => {
                    ranges_unresolved.push(Range::Bounded(IntVal::Const(l), IntVal::Const(u)))
                }
                Range::UnboundedL(l) => ranges_unresolved.push(Range::UnboundedL(IntVal::Const(l))),
                Range::UnboundedR(u) => ranges_unresolved.push(Range::UnboundedR(IntVal::Const(u))),
                Range::Unbounded => ranges_unresolved.push(Range::Unbounded),
            }
        }
        return Ok(Some(Domain::int(ranges_unresolved)));
    }

    Ok(Some(Domain::int(ranges)))
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
    let index_domain_list = field!(matrix_domain, "index_domain_list");
    for domain in named_children(&index_domain_list) {
        let Some(parsed_domain) = parse_domain(ctx, domain)? else {
            return Ok(None);
        };
        domains.push(parsed_domain);
    }
    let Some(value_domain) = parse_domain(ctx, field!(matrix_domain, "value_domain"))? else {
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
        let name_node = field!(record_entry, "name");
        let name = Name::user(&ctx.source_code[name_node.start_byte()..name_node.end_byte()]);
        let domain_node = field!(record_entry, "domain");
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
                    let min_val = parse_int(ctx, &min_node)?;
                    let max_val = parse_int(ctx, &max_node)?;

                    set_attribute = Some(SetAttr::new_min_max_size(min_val, max_val));
                } else if let Some(size_node) = size_value_node {
                    // Size case
                    let size_val = parse_int(ctx, &size_node)?;
                    set_attribute = Some(SetAttr::new_size(size_val));
                } else if let Some(min_node) = min_value_node {
                    // MinSize only case
                    let min_val = parse_int(ctx, &min_node)?;
                    set_attribute = Some(SetAttr::new_min_size(min_val));
                } else if let Some(max_node) = max_value_node {
                    // MaxSize only case
                    let max_val = parse_int(ctx, &max_node)?;
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

use super::atom::parse_int;
use super::util::named_children;
use crate::diagnostics::diagnostics_api::SymbolKind;
use crate::diagnostics::source_map::{HoverInfo, span_with_hover};
use crate::errors::FatalParseError;
use crate::expression::parse_expression;
use crate::parser::ParseContext;
use crate::util::TypecheckingContext;
use crate::{RecoverableParseError, child};
use conjure_cp_core::ast::{
    Atom, BinaryAttr, DeclarationPtr, Domain, DomainPtr, Expression, Field, FuncAttr, IntVal,
    JectivityAttr, Literal, MSetAttr, Moo, Name, PartialityAttr, PartitionAttr, Range, Reference,
    RelAttr, SequenceAttr, SetAttr,
};
use tree_sitter::Node;

use crate::field;

/// Parse an Essence variable domain into its Conjure AST representation.
pub fn parse_domain(
    ctx: &mut ParseContext,
    domain: Node,
) -> Result<Option<DomainPtr>, FatalParseError> {
    match domain.kind() {
        "domain" | "annotation_domain" => {
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
            ctx.add_span_and_doc_hover(&domain, "L_bool", SymbolKind::Domain, None, None);
            Ok(Some(Domain::bool()))
        }
        "int_domain" | "annotation_int_domain" => parse_int_domain(ctx, domain),
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

            // Not form docs, because we need context specific hover info
            span_with_hover(
                &domain,
                ctx.source_code,
                ctx.source_map,
                HoverInfo {
                    description: format!("Domain reference: {name}"),
                    doc_key: None,
                    kind: Some(SymbolKind::Variable),
                    ty: None,
                    decl_span: None, // could link to the declaration span if we wanted
                },
            );
            Ok(Some(dom))
        }
        "tuple_domain" | "annotation_tuple_domain" => parse_tuple_domain(ctx, domain),
        "matrix_domain" | "annotation_matrix_domain" => parse_matrix_domain(ctx, domain),
        "record_domain" | "annotation_record_domain" => parse_record_domain(ctx, domain),
        "variant_domain" | "annotation_variant_domain" => parse_variant_domain(ctx, domain),
        "set_domain" | "annotation_set_domain" => parse_set_domain(ctx, domain),
        "mset_domain" | "annotation_mset_domain" => parse_mset_domain(ctx, domain),
        "sequence_domain" | "annotation_sequence_domain" => parse_sequence_domain(ctx, domain),
        "function_domain" | "annotation_function_domain" => parse_function_domain(ctx, domain),
        "relation_domain" => parse_relation_domain(ctx, domain),
        "partition_domain" => parse_partition_domain(ctx, domain),
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("{} is not a supported domain type", domain.kind()),
                Some(domain.range()),
            ));
            Ok(None)
        }
    }
}

fn parse_mset_domain(
    ctx: &mut ParseContext,
    mset_domain: Node,
) -> Result<Option<DomainPtr>, FatalParseError> {
    let _representation = mset_domain
        .child_by_field_name("representation")
        .map(|node| ctx.source_code[node.start_byte()..node.end_byte()].to_string());
    let mut size = Range::Unbounded;
    let mut occurrence = Range::Unbounded;
    let mut min_size = None;
    let mut max_size = None;
    let mut min_occurrence = None;
    let mut max_occurrence = None;
    let mut value_domain = None;

    for child in named_children(&mset_domain) {
        match child.kind() {
            "identifier" => {}
            "mset_attributes" => {
                for attribute in named_children(&child) {
                    let Some(value_node) = attribute.child_by_field_name("value") else {
                        return Ok(None);
                    };
                    let Some(value) = parse_int(ctx, &value_node) else {
                        return Ok(None);
                    };
                    let name = attribute
                        .child_by_field_name("attribute")
                        .map(|node| &ctx.source_code[node.start_byte()..node.end_byte()])
                        .unwrap_or_default();
                    match name {
                        "size" => size = Range::Single(value),
                        "minSize" => min_size = Some(value),
                        "maxSize" => max_size = Some(value),
                        "minOccur" => min_occurrence = Some(value),
                        "maxOccur" => max_occurrence = Some(value),
                        _ => return Ok(None),
                    }
                }
            }
            "domain" | "annotation_domain" => value_domain = parse_domain(ctx, child)?,
            _ => {}
        }
    }

    if !matches!(size, Range::Single(_)) {
        size = match (min_size, max_size) {
            (Some(min), Some(max)) => Range::Bounded(min, max),
            (Some(min), None) => Range::UnboundedR(min),
            (None, Some(max)) => Range::UnboundedL(max),
            (None, None) => Range::Unbounded,
        };
    }
    occurrence = match (min_occurrence, max_occurrence) {
        (Some(min), Some(max)) => Range::Bounded(min, max),
        (Some(min), None) => Range::UnboundedR(min),
        (None, Some(max)) => Range::UnboundedL(max),
        (None, None) => occurrence,
    };

    let Some(value_domain) = value_domain else {
        ctx.record_error(RecoverableParseError::new(
            "Multiset domain must have a value domain".to_string(),
            Some(mset_domain.range()),
        ));
        return Ok(None);
    };
    let attrs = MSetAttr::new(size, occurrence);
    Ok(Some(Domain::mset(attrs, value_domain)))
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
    let int_keyword_node = child!(int_domain, 0, "int");

    let Some(range_list) = int_domain.child_by_field_name("ranges") else {
        ctx.add_span_and_doc_hover(&int_keyword_node, "L_int", SymbolKind::Domain, None, None);
        let int_text = ctx.source_code[int_domain.start_byte()..int_domain.end_byte()].trim();
        return if int_text == "int" {
            Ok(Some(Domain::int(vec![Range::Bounded(
                conjure_cp_core::ast::OXIDE_INT_MIN,
                conjure_cp_core::ast::OXIDE_INT_MAX,
            )])))
        } else {
            Ok(Some(Domain::int_ground(vec![])))
        };
    };
    let mut ranges_unresolved: Vec<Range<IntVal>> = Vec::new();
    let mut all_resolved = true;

    for domain_component in named_children(&range_list) {
        match domain_component.kind() {
            "atom" | "arithmetic_expr" | "integer" | "identifier" => {
                let Some(int_val) = parse_int_val(ctx, domain_component)? else {
                    return Ok(None);
                };

                if !matches!(int_val, IntVal::Const(_)) {
                    all_resolved = false;
                }
                ranges_unresolved.push(Range::Single(int_val));
            }
            "int_range" | "annotation_int_range" => {
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
                        if !matches!((&lower, &upper), (IntVal::Const(_), IntVal::Const(_))) {
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
            .map(|r| r.resolve())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                FatalParseError::internal_error(
                    format!("could not resolve range: {e}"),
                    Some(int_domain.range()),
                )
            })?
            .into_iter()
            .filter(|range| !matches!(range, Range::Bounded(lower, upper) if lower > upper))
            .collect();

        ctx.add_span_and_doc_hover(&int_keyword_node, "L_int", SymbolKind::Domain, None, None);
        Ok(Some(Domain::int(ranges)))
    } else {
        // Otherwise, keep as an expression-based domain

        // Adding int keyword to the source map with hover info from documentation
        ctx.add_span_and_doc_hover(&int_keyword_node, "L_int", SymbolKind::Domain, None, None);
        Ok(Some(Domain::int(ranges_unresolved)))
    }
}

// Helper function to parse a node into an IntVal
// Handles constants, references, and arbitrary expressions
fn parse_int_val(ctx: &mut ParseContext, node: Node) -> Result<Option<IntVal>, FatalParseError> {
    if matches!(node.kind(), "atom" | "integer") {
        let text = &ctx.source_code[node.start_byte()..node.end_byte()];
        if let Ok(integer) = text.parse::<i32>() {
            return Ok(Some(IntVal::new_const(integer)));
        }
    }

    if node.kind() == "identifier" {
        let Some(decl) = get_declaration_ptr_from_identifier(ctx, node)? else {
            // If identifier isn't defined, it's a semantic error.
            return Ok(None);
        };
        return Ok(Some(IntVal::Reference(Reference::new(decl))));
    }

    let saved_context = ctx.typechecking_context;
    let saved_inner_context = ctx.inner_typechecking_context;
    ctx.typechecking_context = TypecheckingContext::Arithmetic;
    ctx.inner_typechecking_context = TypecheckingContext::Arithmetic;
    let expr = parse_expression(ctx, node)?;
    ctx.typechecking_context = saved_context;
    ctx.inner_typechecking_context = saved_inner_context;

    let Some(expr) = expr else {
        return Ok(None);
    };

    if let Expression::Atomic(_, Atom::Reference(reference)) = &expr
        && let Ok(reference_val) = IntVal::new_ref(reference)
    {
        return Ok(Some(reference_val));
    }

    if let Expression::Atomic(_, Atom::Literal(Literal::Int(value))) = expr {
        return Ok(Some(IntVal::new_const(value)));
    }

    Ok(IntVal::new_expr(Moo::new(expr)).ok())
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

    // extract the first child node which should be the 'tuple' keyword for hover info
    if let Some(first) = tuple_domain.child(0)
        && first.kind() == "tuple"
    {
        // Adding tuple to the source map with hover info from documentation
        ctx.add_span_and_doc_hover(&first, "L_tuple", SymbolKind::Domain, None, None);
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

    // Adding matrix to the source map with hover info from documentation
    let matrix_keyword_node = child!(matrix_domain, 0, "matrix");
    ctx.add_span_and_doc_hover(
        &matrix_keyword_node,
        "matrix",
        SymbolKind::Domain,
        None,
        None,
    );
    Ok(Some(Domain::matrix(value_domain, domains)))
}

fn parse_record_domain(
    ctx: &mut ParseContext,
    record_domain: Node,
) -> Result<Option<DomainPtr>, FatalParseError> {
    let mut record_entries: Vec<Field<DomainPtr>> = Vec::new();
    for record_entry in named_children(&record_domain) {
        let Some(name_node) = field!(recover, ctx, record_entry, "name") else {
            return Ok(None);
        };
        let name = Name::user(&ctx.source_code[name_node.start_byte()..name_node.end_byte()]);
        let Some(domain_node) = field!(recover, ctx, record_entry, "domain") else {
            return Ok(None);
        };
        let Some(value) = parse_domain(ctx, domain_node)? else {
            return Ok(None);
        };
        record_entries.push(Field { name, value });
    }

    // Adding record keyword to the source map with hover info from documentation
    let record_keyword_node = child!(record_domain, 0, "record");
    ctx.add_span_and_doc_hover(
        &record_keyword_node,
        "L_record",
        SymbolKind::Domain,
        None,
        None,
    );
    Ok(Some(Domain::record(record_entries)))
}

fn parse_variant_domain(
    ctx: &mut ParseContext,
    variant_domain: Node,
) -> Result<Option<DomainPtr>, FatalParseError> {
    let mut entries = Vec::new();
    for entry in named_children(&variant_domain) {
        let Some(name_node) = field!(recover, ctx, entry, "name") else {
            return Ok(None);
        };
        let name = Name::user(&ctx.source_code[name_node.start_byte()..name_node.end_byte()]);
        let Some(domain_node) = field!(recover, ctx, entry, "domain") else {
            return Ok(None);
        };
        let Some(value) = parse_domain(ctx, domain_node)? else {
            return Ok(None);
        };
        entries.push(Field { name, value });
    }

    let keyword = child!(variant_domain, 0, "variant");
    ctx.add_span_and_doc_hover(&keyword, "variant", SymbolKind::Domain, None, None);
    Ok(Some(Domain::variant(entries)))
}

fn parse_sequence_domain(
    ctx: &mut ParseContext,
    sequence_domain: Node,
) -> Result<Option<DomainPtr>, FatalParseError> {
    let mut size = Range::Unbounded;
    let mut min_size = None;
    let mut max_size = None;
    let mut jectivity = JectivityAttr::None;
    let mut value_domain: Option<DomainPtr> = None;

    for child in named_children(&sequence_domain) {
        match child.kind() {
            "sequence_attributes" => {
                for attribute in named_children(&child) {
                    let name = attribute
                        .child_by_field_name("attribute")
                        .map(|node| &ctx.source_code[node.start_byte()..node.end_byte()])
                        .unwrap_or_default();
                    match name {
                        "size" | "minSize" | "maxSize" => {
                            let Some(value_node) = attribute.child_by_field_name("value") else {
                                return Ok(None);
                            };
                            let Some(value) = parse_int(ctx, &value_node) else {
                                return Ok(None);
                            };
                            match name {
                                "size" => size = Range::Single(value),
                                "minSize" => min_size = Some(value),
                                "maxSize" => max_size = Some(value),
                                _ => unreachable!(),
                            }
                        }
                        "injective" => jectivity = JectivityAttr::Injective,
                        "surjective" => jectivity = JectivityAttr::Surjective,
                        "bijective" => jectivity = JectivityAttr::Bijective,
                        _ => return Ok(None),
                    }
                }
            }
            "domain" | "annotation_domain" => value_domain = parse_domain(ctx, child)?,
            _ => {}
        }
    }

    if !matches!(size, Range::Single(_)) {
        size = match (min_size, max_size) {
            (Some(min), Some(max)) => Range::Bounded(min, max),
            (Some(min), None) => Range::UnboundedR(min),
            (None, Some(max)) => Range::UnboundedL(max),
            (None, None) => Range::Unbounded,
        };
    }

    let Some(value_domain) = value_domain else {
        ctx.record_error(RecoverableParseError::new(
            "Sequence domain must have a value domain".to_string(),
            Some(sequence_domain.range()),
        ));
        return Ok(None);
    };

    // Adding sequence keyword to the source map with hover info from documentation
    let sequence_keyword_node = child!(sequence_domain, 0, "sequence");
    ctx.add_span_and_doc_hover(
        &sequence_keyword_node,
        "sequence",
        SymbolKind::Domain,
        None,
        None,
    );

    let attrs = SequenceAttr {
        size,
        jectivity,
        representation: None,
    };
    Ok(Some(Domain::sequence(attrs, value_domain)))
}

// e.g. function int(1..3) --> int(1..3), function (total) bool --> int(13,17),
// function (minSize 1) int(1..2) --> set (size 1) of int(1..2). Mirrors
// parse_sequence_domain, plus the extra `partiality` field FuncAttr has.
fn parse_function_domain(
    ctx: &mut ParseContext,
    function_domain: Node,
) -> Result<Option<DomainPtr>, FatalParseError> {
    let mut size = Range::Unbounded;
    let mut min_size = None;
    let mut max_size = None;
    let mut partiality = PartialityAttr::Partial;
    let mut jectivity = JectivityAttr::None;

    for child in named_children(&function_domain) {
        if child.kind() == "function_attributes" {
            for attribute in named_children(&child) {
                let name = attribute
                    .child_by_field_name("attribute")
                    .map(|node| &ctx.source_code[node.start_byte()..node.end_byte()])
                    .unwrap_or_default();
                match name {
                    "size" | "minSize" | "maxSize" => {
                        let Some(value_node) = attribute.child_by_field_name("value") else {
                            return Ok(None);
                        };
                        let Some(value) = parse_int(ctx, &value_node) else {
                            return Ok(None);
                        };
                        match name {
                            "size" => size = Range::Single(value),
                            "minSize" => min_size = Some(value),
                            "maxSize" => max_size = Some(value),
                            _ => unreachable!(),
                        }
                    }
                    "total" => partiality = PartialityAttr::Total,
                    "injective" => jectivity = JectivityAttr::Injective,
                    "surjective" => jectivity = JectivityAttr::Surjective,
                    "bijective" => jectivity = JectivityAttr::Bijective,
                    _ => return Ok(None),
                }
            }
        }
    }

    if !matches!(size, Range::Single(_)) {
        size = match (min_size, max_size) {
            (Some(min), Some(max)) => Range::Bounded(min, max),
            (Some(min), None) => Range::UnboundedR(min),
            (None, Some(max)) => Range::UnboundedL(max),
            (None, None) => Range::Unbounded,
        };
    }

    let Some(domain_from_node) = function_domain.child_by_field_name("domain_from") else {
        ctx.record_error(RecoverableParseError::new(
            "Function domain must have a domain".to_string(),
            Some(function_domain.range()),
        ));
        return Ok(None);
    };
    let Some(domain_from) = parse_domain(ctx, domain_from_node)? else {
        return Ok(None);
    };

    let Some(domain_to_node) = function_domain.child_by_field_name("domain_to") else {
        ctx.record_error(RecoverableParseError::new(
            "Function domain must have a codomain".to_string(),
            Some(function_domain.range()),
        ));
        return Ok(None);
    };
    let Some(domain_to) = parse_domain(ctx, domain_to_node)? else {
        return Ok(None);
    };

    // Adding function keyword to the source map with hover info from documentation
    let function_keyword_node = child!(function_domain, 0, "function");
    ctx.add_span_and_doc_hover(
        &function_keyword_node,
        "function",
        SymbolKind::Domain,
        None,
        None,
    );

    let attrs = FuncAttr {
        size,
        partiality,
        jectivity,
    };
    Ok(Some(Domain::function(attrs, domain_from, domain_to)))
}

// e.g. relation of (int(1..3) * bool), relation (size 2, irreflexive) of (int(0..5) * int(0..5)).
// Binary relation attributes only make sense for a binary relation whose two columns share a
// domain; that constraint is enforced by the representations (`init`), not the parser.
fn parse_relation_domain(
    ctx: &mut ParseContext,
    relation_domain: Node,
) -> Result<Option<DomainPtr>, FatalParseError> {
    let mut size = Range::Unbounded;
    let mut min_size = None;
    let mut max_size = None;
    let mut binary = Vec::new();

    for child in named_children(&relation_domain) {
        if child.kind() == "relation_attributes" {
            for attribute in named_children(&child) {
                let name = attribute
                    .child_by_field_name("attribute")
                    .map(|node| &ctx.source_code[node.start_byte()..node.end_byte()])
                    .unwrap_or_default();
                match name {
                    "size" | "minSize" | "maxSize" => {
                        let Some(value_node) = attribute.child_by_field_name("value") else {
                            return Ok(None);
                        };
                        let Some(value) = parse_int(ctx, &value_node) else {
                            return Ok(None);
                        };
                        match name {
                            "size" => size = Range::Single(value),
                            "minSize" => min_size = Some(value),
                            "maxSize" => max_size = Some(value),
                            _ => unreachable!(),
                        }
                    }
                    _ => {
                        let Some(bin_attr) = BinaryAttr::from_keyword(name) else {
                            return Ok(None);
                        };
                        binary.push(bin_attr);
                    }
                }
            }
        }
    }

    if !matches!(size, Range::Single(_)) {
        size = match (min_size, max_size) {
            (Some(min), Some(max)) => Range::Bounded(min, max),
            (Some(min), None) => Range::UnboundedR(min),
            (None, Some(max)) => Range::UnboundedL(max),
            (None, None) => Range::Unbounded,
        };
    }

    let Some(column_list) = field!(recover, ctx, relation_domain, "relation_domain_list") else {
        return Ok(None);
    };
    let mut columns: Vec<DomainPtr> = Vec::new();
    for column in named_children(&column_list) {
        let Some(parsed) = parse_domain(ctx, column)? else {
            return Ok(None);
        };
        columns.push(parsed);
    }

    // Adding relation keyword to the source map with hover info from documentation
    let relation_keyword_node = child!(relation_domain, 0, "relation");
    ctx.add_span_and_doc_hover(
        &relation_keyword_node,
        "relation",
        SymbolKind::Domain,
        None,
        None,
    );

    let attrs = RelAttr { size, binary };
    Ok(Some(Domain::relation(attrs, columns)))
}

fn parse_partition_domain(
    ctx: &mut ParseContext,
    partition_domain: Node,
) -> Result<Option<DomainPtr>, FatalParseError> {
    let mut num_parts = Range::Unbounded;
    let mut min_num_parts = None;
    let mut max_num_parts = None;
    let mut part_len = Range::Unbounded;
    let mut min_part_len = None;
    let mut max_part_len = None;
    let mut is_regular = false;

    for child in named_children(&partition_domain) {
        if child.kind() == "partition_attributes" {
            for attribute in named_children(&child) {
                let name = attribute
                    .child_by_field_name("attribute")
                    .map(|node| &ctx.source_code[node.start_byte()..node.end_byte()])
                    .unwrap_or_default();
                match name {
                    "numParts" | "minNumParts" | "maxNumParts" | "partSize" | "minPartSize"
                    | "maxPartSize" => {
                        let Some(value_node) = attribute.child_by_field_name("value") else {
                            return Ok(None);
                        };
                        let Some(value) = parse_int(ctx, &value_node) else {
                            return Ok(None);
                        };
                        match name {
                            "numParts" => num_parts = Range::Single(value),
                            "minNumParts" => min_num_parts = Some(value),
                            "maxNumParts" => max_num_parts = Some(value),
                            "partSize" => part_len = Range::Single(value),
                            "minPartSize" => min_part_len = Some(value),
                            "maxPartSize" => max_part_len = Some(value),
                            _ => unreachable!(),
                        }
                    }
                    "regular" => is_regular = true,
                    _ => return Ok(None),
                }
            }
        }
    }

    if !matches!(num_parts, Range::Single(_)) {
        num_parts = match (min_num_parts, max_num_parts) {
            (Some(min), Some(max)) => Range::Bounded(min, max),
            (Some(min), None) => Range::UnboundedR(min),
            (None, Some(max)) => Range::UnboundedL(max),
            (None, None) => Range::Unbounded,
        };
    }
    if !matches!(part_len, Range::Single(_)) {
        part_len = match (min_part_len, max_part_len) {
            (Some(min), Some(max)) => Range::Bounded(min, max),
            (Some(min), None) => Range::UnboundedR(min),
            (None, Some(max)) => Range::UnboundedL(max),
            (None, None) => Range::Unbounded,
        };
    }

    let Some(value_domain) = field!(recover, ctx, partition_domain, "value_domain") else {
        return Ok(None);
    };
    let Some(inner) = parse_domain(ctx, value_domain)? else {
        return Ok(None);
    };

    let partition_keyword_node = child!(partition_domain, 0, "partition");
    ctx.add_span_and_doc_hover(
        &partition_keyword_node,
        "partition",
        SymbolKind::Domain,
        None,
        None,
    );

    let attrs = PartitionAttr {
        num_parts,
        part_len,
        is_regular,
    };
    Ok(Some(Domain::partition(attrs, inner)))
}

pub fn parse_set_domain(
    ctx: &mut ParseContext,
    set_domain: Node,
) -> Result<Option<DomainPtr>, FatalParseError> {
    let mut set_attribute: Option<SetAttr> = None;
    let mut value_domain: Option<DomainPtr> = None;

    let representation = set_domain
        .child_by_field_name("representation")
        .map(|node| ctx.source_code[node.start_byte()..node.end_byte()].to_string());

    for child in named_children(&set_domain) {
        match child.kind() {
            "identifier" => {
                // Representation preference is handled via the `representation` field above.
            }
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
            "domain" | "annotation_domain" => {
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
        // Adding set to the source map with hover info from documentation
        let set_keyword_node = child!(set_domain, 0, "set");
        // No documentation available for set domain, using fallback description
        ctx.add_span_and_doc_hover(&set_keyword_node, "set", SymbolKind::Domain, None, None);
        let mut attr = set_attribute.unwrap_or_default();
        if let Some(repr) = representation {
            attr = attr.with_representation(repr);
        }
        Ok(Some(Domain::set(attr, domain)))
    } else {
        ctx.record_error(RecoverableParseError::new(
            "Set domain must have a value domain".to_string(),
            Some(set_domain.range()),
        ));
        Ok(None)
    }
}

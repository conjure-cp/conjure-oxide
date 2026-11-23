use std::collections::HashSet;

use crate::expression::parse_expression;
use crate::parser::ParseContext;
use crate::{errors::FatalParseError, parser::parse_ident};
use conjure_cp_core::ast::{
    BinaryAttr, Domain, DomainPtr, Field, FuncAttr, IntVal, JectivityAttr, MSetAttr, Moo, Name,
    PartialityAttr, PartitionAttr, PermutationAttr, Range, RelAttr, SequenceAttr, SetAttr,
};
use tree_sitter::Node;

use crate::field;

fn parse_intval(ctx: &mut ParseContext<'_>, node: Node<'_>) -> Result<IntVal, FatalParseError> {
    Ok(IntVal::Expr(Moo::new(parse_expression(ctx, node)?)))
}

pub fn parse_ranges(
    ctx: &mut ParseContext,
    node: Node,
) -> Result<Vec<Range<IntVal>>, FatalParseError> {
    node.named_children(&mut node.walk())
        .map(|n| {
            Ok(if let Some(range_node) = n.child_by_field_name("range") {
                Range::new(
                    range_node
                        .child_by_field_name("lower")
                        .map(|ln| parse_intval(ctx, ln))
                        .transpose()?,
                    range_node
                        .child_by_field_name("upper")
                        .map(|un| parse_intval(ctx, un))
                        .transpose()?,
                )
            } else if let Some(value_node) = n.child_by_field_name("value") {
                Range::Single(IntVal::Expr(Moo::new(parse_expression(ctx, value_node)?)))
            } else {
                unreachable!()
            })
        })
        .collect()
}
/// Parse an Essence variable domain into its Conjure AST representation.
pub fn parse_domain(ctx: &mut ParseContext, node: Node) -> Result<DomainPtr, FatalParseError> {
    let domain = match node.kind() {
        "bool_domain" => Domain::bool(),
        "int_domain" => {
            let ranges = node
                .child_by_field_name("ranges")
                .map(|n| parse_ranges(ctx, n))
                .transpose()?
                .unwrap_or_default();

            Domain::int(ranges)
        }
        "tuple_domain" => {
            let domains = node
                .named_children(&mut node.walk())
                .map(|n| parse_domain(ctx, n))
                .collect::<Result<_, FatalParseError>>()?;

            Domain::tuple(domains)
        }
        "matrix_domain" => {
            let index_domains = node
                .children_by_field_name("index_domain_list", &mut node.walk())
                .map(|n| parse_domain(ctx, n))
                .collect::<Result<_, FatalParseError>>()?;

            let value_domain = parse_domain(ctx, field!(node, "value_domain"))?;

            Domain::matrix(value_domain, index_domains)
        }
        #[rustfmt::skip]
        "set_domain" => {
            let mut size = Range::Unbounded;
            let mut representation = None;

            for (attr_name, value) in walk_domain_attributes(ctx, node)? {
                match dbg!(attr_name.as_ref()) {
                    "size" => { size = set_range_attr(size, ctx, value, RangeValue::Exact)?; }
                    "minSize" => { size = set_range_attr(size, ctx, value, RangeValue::Min)?; }
                    "maxSize" => { size = set_range_attr(size, ctx, value, RangeValue::Max)?; }
                    "representation" => set_repr_attr(&mut representation, ctx, value)?,
                    _ => todo!(),
                };
            }

            let value_domain = parse_domain(ctx, field!(node, "value_domain"))?;

            let attrs = SetAttr {
                size,
                representation,
            };
            Domain::set(attrs, value_domain)
        }
        #[rustfmt::skip]
        "mset_domain" => {
            let mut size = Range::Unbounded;
            let mut occur = Range::Unbounded;
            let mut representation = None;

            for (attr_name, value) in walk_domain_attributes(ctx, node)? {
                match attr_name.as_ref() {
                    "size" => { size = set_range_attr(size, ctx, value, RangeValue::Exact)?; }
                    "minSize" => { size = set_range_attr(size, ctx, value, RangeValue::Min)?; }
                    "maxSize" => { size = set_range_attr(size, ctx, value, RangeValue::Max)?; }
                    "minOccur" => { occur = set_range_attr(occur, ctx, value, RangeValue::Min)?; }
                    "maxOccur" => { occur = set_range_attr(occur, ctx, value, RangeValue::Max)?; }
                    "representation" => set_repr_attr(&mut representation, ctx, value)?,
                    _ => todo!(),
                };
            }

            let value_domain = parse_domain(ctx, field!(node, "value_domain"))?;

            let attrs = MSetAttr::new(size, occur);
            Domain::mset(attrs, value_domain)
        }
        #[rustfmt::skip]
        "sequence_domain" => {
            let mut size = Range::Unbounded;
            let mut jectivity = JectivityAttr::None;
            let mut representation = None;

            for (attr_name, value) in walk_domain_attributes(ctx, node)? {
                match attr_name.as_ref() {
                    "size" => { size = set_range_attr(size, ctx, value, RangeValue::Exact)?; }
                    "minSize" => { size = set_range_attr(size, ctx, value, RangeValue::Min)?; }
                    "maxSize" => { size = set_range_attr(size, ctx, value, RangeValue::Max)?; }
                    "bijective" => set_jectivity_attr(&mut jectivity, JectivityAttr::Bijective)?,
                    "surjective" => set_jectivity_attr(&mut jectivity, JectivityAttr::Surjective)?,
                    "injective" => set_jectivity_attr(&mut jectivity, JectivityAttr::Injective)?,
                    "representation" => {
                        set_repr_attr(&mut representation, ctx, value)?
                    }
                    _ => todo!(),
                }
            }
            let value_domain = parse_domain(ctx, field!(node, "value_domain"))?;
            let attrs = SequenceAttr {
                size,
                jectivity,
                representation,
            };
            Domain::sequence(attrs, value_domain)
        }
        #[rustfmt::skip]
        "function_domain" => {
            let mut size = Range::Unbounded;
            let mut jectivity = JectivityAttr::None;
            let mut partiality = PartialityAttr::Partial;

            for (attr_name, value) in walk_domain_attributes(ctx, node)? {
                match attr_name.as_ref() {
                    "size" => { size = set_range_attr(size, ctx, value, RangeValue::Exact)?; }
                    "minSize" => { size = set_range_attr(size, ctx, value, RangeValue::Min)?; }
                    "maxSize" => { size = set_range_attr(size, ctx, value, RangeValue::Max)?; }
                    "bijective" => set_jectivity_attr(&mut jectivity, JectivityAttr::Bijective)?,
                    "surjective" => set_jectivity_attr(&mut jectivity, JectivityAttr::Surjective)?,
                    "injective" => set_jectivity_attr(&mut jectivity, JectivityAttr::Injective)?,
                    "total" => { partiality = PartialityAttr::Total; }
                    _ => todo!(),
                }
            }
            let input_domain = parse_domain(ctx, field!(node, "domain_from"))?;
            let output_domain = parse_domain(ctx, field!(node, "domain_to"))?;
            let attrs = FuncAttr {
                size,
                jectivity,
                partiality,
            };

            Domain::function(attrs, input_domain, output_domain)
        }
        #[rustfmt::skip]
        "relation_domain" => {
            let mut size = Range::Unbounded;
            let mut binary = HashSet::new();

            for (attr_name, value) in walk_domain_attributes(ctx, node)? {
                match attr_name.as_ref() {
                    "size" => { size = set_range_attr(size, ctx, value, RangeValue::Exact)?; }
                    "minSize" => { size = set_range_attr(size, ctx, value, RangeValue::Min)?; }
                    "maxSize" => { size = set_range_attr(size, ctx, value, RangeValue::Max)?; }
                    "reflexive" => { binary.insert(BinaryAttr::Reflexive); },
                    "irreflexive" => { binary.insert(BinaryAttr::Irreflexive); },
                    "coreflexive" => {  binary.insert(BinaryAttr::Coreflexive); }
                    "symmetric" => {  binary.insert(BinaryAttr::Symmetric); }
                    "antiSymmetric" => {  binary.insert(BinaryAttr::Symmetric); }
                    "aSymmetric"=> {  binary.insert(BinaryAttr::ASymmetric); }
                    "transitive"=> {  binary.insert(BinaryAttr::Transitive); }
                    "total"=> {  binary.insert(BinaryAttr::Total); }
                    "connex"=> {  binary.insert(BinaryAttr::Connex); }
                    "Euclidean"=> {  binary.insert(BinaryAttr::Euclidean); }
                    "serial"=> {  binary.insert(BinaryAttr::Serial); }
                    "equivalence"=> {  binary.insert(BinaryAttr::Equivalence); }
                    "partialOrder"=> {  binary.insert(BinaryAttr::PartialOrder); }
                    "leftTotal"=> {  binary.insert(BinaryAttr::LeftTotal); }
                    "rightTotal"=> {  binary.insert(BinaryAttr::RightTotal); }
                    "linearOrder"=> {  binary.insert(BinaryAttr::LinearOrder); }
                    "weakOrder"=> {  binary.insert(BinaryAttr::WeakOrder); }
                    "preOrder"=> {  binary.insert(BinaryAttr::PreOrder); }
                    "strictPartialOrder"=> {  binary.insert(BinaryAttr::StrictPartialOrder); }
                    _ => todo!(),
                }
            }

            let domains = node
                .children_by_field_name("relation_domain_list", &mut node.walk())
                .map(|n| parse_domain(ctx, n))
                .collect::<Result<_, FatalParseError>>()?;

            let attrs = RelAttr {
                size,
                binary: binary.into_iter().collect(),
            };

            Domain::relation(attrs, domains)
        }
        #[rustfmt::skip]
        "partition_domain" => {
            let mut num_parts = Range::Unbounded;
            let mut part_len = Range::Unbounded;
            let mut is_regular = false;

            for (attr_name, value) in walk_domain_attributes(ctx, node)? {
                match attr_name.as_ref() {
                    "numParts" => { num_parts = set_range_attr(num_parts, ctx, value, RangeValue::Exact)?; }
                    "minNumParts" => { num_parts = set_range_attr(num_parts, ctx, value, RangeValue::Min)?; }
                    "maxNumParts" => { num_parts = set_range_attr(num_parts, ctx, value, RangeValue::Max)?; }
                    "partSize" => { part_len = set_range_attr(part_len, ctx, value, RangeValue::Exact)?; }
                    "minPartSize" => { part_len = set_range_attr(part_len, ctx, value, RangeValue::Min)?; }
                    "maxPartSize" => { part_len = set_range_attr(part_len, ctx, value, RangeValue::Max)?; }
                    "regular" => { is_regular = true; }
                    _ => todo!(),
                }
            }

            let value_domain = parse_domain(ctx, field!(node, "value_domain"))?;

            let attrs = PartitionAttr {
                num_parts,
                part_len,
                is_regular,
            };
            Domain::partition(attrs, value_domain)
        }
        #[rustfmt::skip]
        "permutation_domain" => {
            let mut num_moved = Range::Unbounded;

            for (attr_name, value) in walk_domain_attributes(ctx, node)? {
                match attr_name.as_ref() {
                    "numMoved" => { num_moved = set_range_attr(num_moved, ctx, value, RangeValue::Exact)?; }
                    "minNumMoved" => { num_moved = set_range_attr(num_moved, ctx, value, RangeValue::Min)?; }
                    "maxNumMoved" => { num_moved = set_range_attr(num_moved, ctx, value, RangeValue::Max)?; }
                    _ => todo!(),
                }
            }

            let value_domain = parse_domain(ctx, field!(node, "value_domain"))?;

            let attrs = PermutationAttr { num_moved };
            Domain::permutation(attrs, value_domain)
        }
        "record_domain" => {
            let entries = node
                .named_children(&mut node.walk())
                .map(|n| {
                    Ok::<_, FatalParseError>(Field {
                        name: Name::User(parse_ident(ctx, &field!(n, "name")).into()),
                        value: parse_domain(ctx, field!(n, "domain"))?,
                    })
                })
                .collect::<Result<_, FatalParseError>>()?;
            Domain::record(entries)
        }
        "variant_domain" => {
            let entries = node
                .named_children(&mut node.walk())
                .map(|n| {
                    Ok::<_, FatalParseError>(Field {
                        name: Name::User(parse_ident(ctx, &field!(n, "name")).into()),
                        value: parse_domain(ctx, field!(n, "domain"))?,
                    })
                })
                .collect::<Result<_, FatalParseError>>()?;
            Domain::variant(entries)
        }
        _ => todo!(),
    };

    Ok(domain)
}

enum RangeValue {
    Exact,
    Min,
    Max,
}

fn set_range_attr(
    range: Range<IntVal>,
    ctx: &mut ParseContext,
    node: Option<Node>,
    kind: RangeValue,
) -> Result<Range<IntVal>, FatalParseError> {
    let Some(node) = node else {
        return Err(todo!());
    };

    let value = parse_intval(ctx, node)?;

    Ok(match kind {
        RangeValue::Exact => {
            if range.is_lower_or_upper_bounded() {
                return Err(todo!());
            }
            Range::Single(value)
        }
        RangeValue::Min => match range {
            Range::UnboundedL(ub) => Range::Bounded(value, ub),
            Range::Unbounded => Range::UnboundedL(value),
            _ => return Err(todo!()),
        },
        RangeValue::Max => match range {
            Range::UnboundedR(lb) => Range::Bounded(lb, value),
            Range::Unbounded => Range::UnboundedL(value),
            _ => return Err(todo!()),
        },
    })
}

fn set_repr_attr(
    repr: &mut Option<String>,
    ctx: &mut ParseContext,
    node: Option<Node>,
) -> Result<(), FatalParseError> {
    if repr.is_some() {
        return Err(todo!());
    }
    let Some(node) = node else {
        return Err(todo!());
    };
    *repr = Some(parse_ident(ctx, &node));
    Ok(())
}

fn set_jectivity_attr(
    value: &mut JectivityAttr,
    new_value: JectivityAttr,
) -> Result<(), FatalParseError> {
    *value = match (&value, new_value) {
        (JectivityAttr::None, j) => j,
        (JectivityAttr::Surjective, JectivityAttr::Injective)
        | (JectivityAttr::Injective, JectivityAttr::Surjective) => JectivityAttr::Bijective,
        (_v, _nv) => return Err(todo!()),
    };

    Ok(())
}

fn walk_domain_attributes<'a>(
    ctx: &mut ParseContext,
    node: Node<'a>,
) -> Result<Vec<(String, Option<Node<'a>>)>, FatalParseError> {
    let Some(attributes) = node.child_by_field_name("attrs") else {
        return Ok(vec![]);
    };

    attributes
        .named_children(&mut attributes.walk())
        .map(|attr| {
            let name_node = field!(attr, "attribute");
            let attr_name = &ctx.source_code[name_node.start_byte()..name_node.end_byte()];
            let value_node = attr.child_by_field_name("value");

            Ok((attr_name.to_owned(), value_node))
        })
        .collect()
}

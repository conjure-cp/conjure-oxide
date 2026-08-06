use crate::errors::{FatalParseError, RecoverableParseError};
use crate::expression::parse_expression;
use crate::field;
use crate::parser::ParseContext;
use crate::parser::domain::parse_domain;
use crate::util::{TypecheckingContext, named_children};
use conjure_cp_core::ast::{AbstractLiteral, DomainPtr, Expression};
use conjure_cp_core::{domain_int, range};
use tree_sitter::Node;

pub fn parse_abstract(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    if typecheck_abstract_literal(ctx, node) {
        return Ok(None);
    }

    match node.kind() {
        "record" => parse_record(ctx, node),
        "variant" => parse_variant(ctx, node),
        "tuple" => parse_tuple(ctx, node),
        "matrix" => parse_matrix(ctx, node),
        "set_literal" => parse_set_literal(ctx, node),
        "mset_literal" => parse_mset_literal(ctx, node),
        "sequence_literal" => parse_sequence_literal(ctx, node),
        "function_literal" => parse_function_literal(ctx, node),
        "relation_literal" => parse_relation_literal(ctx, node),
        "partition_literal" => parse_partition_literal(ctx, node),
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Expected abstract literal, got: {}", node.kind()),
                Some(node.range()),
            ));
            Ok(None)
        }
    }
}

fn typecheck_abstract_literal(ctx: &mut ParseContext, node: &Node) -> bool {
    let expected = match ctx.typechecking_context {
        TypecheckingContext::Boolean => "bool",
        TypecheckingContext::Arithmetic => "int",
        TypecheckingContext::Set => "set",
        TypecheckingContext::SetOrMatrix => "set or matrix",
        TypecheckingContext::MSet => "mset",
        TypecheckingContext::Matrix => "matrix",
        TypecheckingContext::Tuple => "tuple",
        TypecheckingContext::Record => "record",
        TypecheckingContext::Partition => "partition",
        TypecheckingContext::Permutation => "permutation",
        TypecheckingContext::Sequence => "sequence",
        TypecheckingContext::Function => "function",
        TypecheckingContext::Relation => "relation",
        TypecheckingContext::Unknown => "unknown",
    };

    let got = match node.kind() {
        "set_literal" => "set",
        "mset_literal" => "mset",
        "sequence_literal" => "sequence",
        "function_literal" => "function",
        "relation_literal" => "relation",
        "partition_literal" => "partition",
        "matrix" => "matrix",
        "tuple" => "tuple",
        "record" => "record",
        "variant" => "variant",
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Expected abstract literal, got: {}", node.kind()),
                Some(node.range()),
            ));
            return true;
        }
    };

    if expected != "unknown"
        && !(ctx.typechecking_context == TypecheckingContext::SetOrMatrix
            && matches!(got, "set" | "matrix"))
        && expected != got
    {
        ctx.record_error(RecoverableParseError::new(
            format!(
                "Type error: {}\n\tExpected: {}\n\tGot: {}",
                ctx.source_code[node.start_byte()..node.end_byte()].trim(),
                expected,
                got
            ),
            Some(node.range()),
        ));
        return true;
    }

    false
}

fn parse_variant(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    let Some(pair) = node.child_by_field_name("name_value_pair") else {
        return Ok(None);
    };
    let Some(name_node) = field!(recover, ctx, pair, "name") else {
        return Ok(None);
    };
    let name = conjure_cp_core::ast::Name::user(
        &ctx.source_code[name_node.start_byte()..name_node.end_byte()],
    );
    let Some(value_node) = field!(recover, ctx, pair, "value") else {
        return Ok(None);
    };
    let saved_context = ctx.typechecking_context;
    let saved_inner_context = ctx.inner_typechecking_context;
    ctx.typechecking_context = ctx.inner_typechecking_context;
    ctx.inner_typechecking_context = TypecheckingContext::Unknown;
    let value = parse_expression(ctx, value_node)?;
    ctx.typechecking_context = saved_context;
    ctx.inner_typechecking_context = saved_inner_context;
    let Some(value) = value else {
        return Ok(None);
    };
    Ok(Some(AbstractLiteral::Variant(
        conjure_cp_core::ast::Moo::new(conjure_cp_core::ast::records::Field { name, value }),
    )))
}

fn parse_record(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    let mut values = Vec::new();
    let mut had_error = false;
    for child in node.children_by_field_name("name_value_pair", &mut node.walk()) {
        let Some(name_node) = field!(recover, ctx, child, "name") else {
            return Ok(None);
        };
        let name_str = &ctx.source_code[name_node.start_byte()..name_node.end_byte()];
        let name = conjure_cp_core::ast::Name::user(name_str);

        let Some(value_node) = field!(recover, ctx, child, "value") else {
            return Ok(None);
        };

        // Parse value with inner typechecking context
        let saved_ctx = ctx.typechecking_context;
        ctx.typechecking_context = ctx.inner_typechecking_context;
        ctx.inner_typechecking_context = TypecheckingContext::Unknown;

        let Some(value) = parse_expression(ctx, value_node)? else {
            had_error = true;
            ctx.typechecking_context = saved_ctx;
            ctx.inner_typechecking_context = TypecheckingContext::Unknown;
            continue; // continue parsing other elements
        };

        // Reset contexts
        ctx.inner_typechecking_context = ctx.typechecking_context;
        ctx.typechecking_context = saved_ctx;

        values.push(conjure_cp_core::ast::records::Field { name, value });
    }

    if had_error {
        Ok(None)
    } else {
        Ok(Some(AbstractLiteral::Record(values)))
    }
}

fn parse_tuple(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    // Save the typechecking context
    let saved_ctx = ctx.typechecking_context;
    let saved_inner_ctx = ctx.inner_typechecking_context;

    let mut elements = Vec::new();
    let mut had_error = false;
    for child in named_children(node) {
        // Parse elements with inner typechecking context
        ctx.typechecking_context = saved_inner_ctx;
        ctx.inner_typechecking_context = TypecheckingContext::Unknown;

        let Some(expr) = parse_expression(ctx, child)? else {
            had_error = true;
            continue;
        };
        elements.push(expr);
    }

    ctx.typechecking_context = saved_ctx;
    ctx.inner_typechecking_context = saved_inner_ctx;
    if had_error {
        Ok(None)
    } else {
        Ok(Some(AbstractLiteral::Tuple(elements)))
    }
}

fn parse_matrix(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    // Save the typechecking contexts
    let saved_ctx = ctx.typechecking_context;
    let saved_inner_ctx = ctx.inner_typechecking_context;

    let mut elements = vec![];
    let mut domain: Option<DomainPtr> = None;
    let mut had_error = false;
    for child in named_children(node) {
        if matches!(child.kind(), "single_line_comment" | "language_declaration") {
            continue;
        }
        if child.kind() == "arithmetic_expr"
            || child.kind() == "bool_expr"
            || child.kind() == "comparison_expr"
            || child.kind() == "atom"
        {
            // Parse elements with inner typechecking context
            ctx.typechecking_context = saved_inner_ctx;
            ctx.inner_typechecking_context = TypecheckingContext::Unknown;

            let Some(expr) = parse_expression(ctx, child)? else {
                had_error = true;
                continue;
            };
            elements.push(expr);
        } else {
            // Parse domains with unknown typechecking context
            ctx.typechecking_context = TypecheckingContext::Unknown;

            let Some(parsed_domain) = parse_domain(ctx, child)? else {
                had_error = true;
                continue;
            };
            domain = Some(parsed_domain);
        }
    }
    if domain.is_none() {
        let count = elements.len() as i32;
        domain = Some(domain_int!(1..count));
    }

    ctx.typechecking_context = saved_ctx;
    ctx.inner_typechecking_context = saved_inner_ctx;
    if had_error {
        Ok(None)
    } else {
        Ok(Some(AbstractLiteral::Matrix(elements, domain.unwrap())))
    }
}

fn parse_set_literal(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    // Save the typechecking contexts
    let saved_ctx = ctx.typechecking_context;
    let saved_inner_ctx = ctx.inner_typechecking_context;

    let mut elements = Vec::new();
    let mut had_error = false;
    for child in named_children(node) {
        // Parse elements with inner typechecking context
        ctx.typechecking_context = saved_inner_ctx;
        ctx.inner_typechecking_context = TypecheckingContext::Unknown;

        let Some(expr) = parse_expression(ctx, child)? else {
            had_error = true;
            continue;
        };
        elements.push(expr);
    }

    ctx.typechecking_context = saved_ctx;
    ctx.inner_typechecking_context = saved_inner_ctx;
    if had_error {
        Ok(None)
    } else {
        Ok(Some(AbstractLiteral::Set(elements)))
    }
}

fn parse_mset_literal(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    let saved_ctx = ctx.typechecking_context;
    let saved_inner_ctx = ctx.inner_typechecking_context;
    let mut elements = Vec::new();
    let mut had_error = false;
    for child in named_children(node) {
        ctx.typechecking_context = saved_inner_ctx;
        ctx.inner_typechecking_context = TypecheckingContext::Unknown;
        let Some(expression) = parse_expression(ctx, child)? else {
            had_error = true;
            continue;
        };
        elements.push(expression);
    }
    ctx.typechecking_context = saved_ctx;
    ctx.inner_typechecking_context = saved_inner_ctx;
    if had_error {
        Ok(None)
    } else {
        Ok(Some(AbstractLiteral::MSet(elements)))
    }
}

fn parse_sequence_literal(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    let saved_ctx = ctx.typechecking_context;
    let saved_inner_ctx = ctx.inner_typechecking_context;
    let mut elements = Vec::new();
    let mut had_error = false;
    for child in named_children(node) {
        ctx.typechecking_context = saved_inner_ctx;
        ctx.inner_typechecking_context = TypecheckingContext::Unknown;
        let Some(expression) = parse_expression(ctx, child)? else {
            had_error = true;
            continue;
        };
        elements.push(expression);
    }
    ctx.typechecking_context = saved_ctx;
    ctx.inner_typechecking_context = saved_inner_ctx;
    if had_error {
        Ok(None)
    } else {
        Ok(Some(AbstractLiteral::Sequence(elements)))
    }
}

fn parse_function_literal(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    let saved_ctx = ctx.typechecking_context;
    let saved_inner_ctx = ctx.inner_typechecking_context;
    let mut pairs = Vec::new();
    let mut had_error = false;
    for pair in named_children(node) {
        // key/value can be of any type (matching the domain/codomain of the function), so
        // typecheck context is relaxed here, mirroring the other abstract literal parsers.
        ctx.typechecking_context = TypecheckingContext::Unknown;
        ctx.inner_typechecking_context = TypecheckingContext::Unknown;

        let Some(key_node) = field!(recover, ctx, pair, "key") else {
            had_error = true;
            continue;
        };
        let Some(key) = parse_expression(ctx, key_node)? else {
            had_error = true;
            continue;
        };

        let Some(value_node) = field!(recover, ctx, pair, "value") else {
            had_error = true;
            continue;
        };
        let Some(value) = parse_expression(ctx, value_node)? else {
            had_error = true;
            continue;
        };

        pairs.push((key, value));
    }
    ctx.typechecking_context = saved_ctx;
    ctx.inner_typechecking_context = saved_inner_ctx;
    if had_error {
        Ok(None)
    } else {
        Ok(Some(AbstractLiteral::Function(pairs)))
    }
}

fn parse_relation_literal(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    let saved_ctx = ctx.typechecking_context;
    let saved_inner_ctx = ctx.inner_typechecking_context;
    let mut tuples = Vec::new();
    let mut had_error = false;
    for tuple_node in named_children(node) {
        let mut fields = Vec::new();
        for field_node in named_children(&tuple_node) {
            // Each column can be of any type (matching the relation's own column domains), so
            // typecheck context is relaxed here, mirroring the other abstract literal parsers.
            ctx.typechecking_context = TypecheckingContext::Unknown;
            ctx.inner_typechecking_context = TypecheckingContext::Unknown;

            let Some(expr) = parse_expression(ctx, field_node)? else {
                had_error = true;
                continue;
            };
            fields.push(expr);
        }
        tuples.push(fields);
    }
    ctx.typechecking_context = saved_ctx;
    ctx.inner_typechecking_context = saved_inner_ctx;
    if had_error {
        Ok(None)
    } else {
        Ok(Some(AbstractLiteral::Relation(tuples)))
    }
}

fn parse_partition_literal(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    let saved_ctx = ctx.typechecking_context;
    let saved_inner_ctx = ctx.inner_typechecking_context;
    let mut parts = Vec::new();
    let mut had_error = false;
    for part_node in named_children(node) {
        // Each part is itself a set literal; its elements can be of any type (matching the
        // partition's own inner domain), so typecheck context is relaxed here, mirroring the
        // other abstract literal parsers.
        ctx.typechecking_context = TypecheckingContext::Unknown;
        ctx.inner_typechecking_context = TypecheckingContext::Unknown;

        let Some(AbstractLiteral::Set(elements)) = parse_set_literal(ctx, &part_node)? else {
            had_error = true;
            continue;
        };
        parts.push(elements);
    }
    ctx.typechecking_context = saved_ctx;
    ctx.inner_typechecking_context = saved_inner_ctx;
    if had_error {
        Ok(None)
    } else {
        Ok(Some(AbstractLiteral::Partition(parts)))
    }
}

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
        "tuple" => parse_tuple(ctx, node),
        "matrix" => parse_matrix(ctx, node),
        "set_literal" => parse_set_literal(ctx, node),
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
        TypecheckingContext::MSet => "mset",
        TypecheckingContext::Matrix => "matrix",
        TypecheckingContext::Tuple => "tuple",
        TypecheckingContext::Record => "record",
        TypecheckingContext::Function => "function",
        TypecheckingContext::Empty => "empty",
        TypecheckingContext::Unknown => "unknown",
    };

    let got = match node.kind() {
        "set_literal" => "set",
        "matrix" => "matrix",
        "tuple" => "tuple",
        "record" => "record",
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Expected abstract literal, got: {}", node.kind()),
                Some(node.range()),
            ));
            return true;
        }
    };

    if expected != "unknown" && expected != got {
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

fn parse_record(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    let mut values = Vec::new();
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
            return Ok(None);
        };

        // Reset contexts
        ctx.inner_typechecking_context = ctx.typechecking_context;
        ctx.typechecking_context = saved_ctx;

        values.push(conjure_cp_core::ast::records::RecordValue { name, value });
    }
    Ok(Some(AbstractLiteral::Record(values)))
}

fn parse_tuple(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    // Parse elements parse under inner_typechecking_context
    let saved_ctx = ctx.typechecking_context;
    ctx.typechecking_context = ctx.inner_typechecking_context;
    ctx.inner_typechecking_context = TypecheckingContext::Unknown;

    let mut elements = Vec::new();
    for child in named_children(node) {
        let Some(expr) = parse_expression(ctx, child)? else {
            ctx.inner_typechecking_context = ctx.typechecking_context;
            ctx.typechecking_context = saved_ctx;
            return Ok(None);
        };
        elements.push(expr);
    }

    ctx.inner_typechecking_context = ctx.typechecking_context;
    ctx.typechecking_context = saved_ctx;
    Ok(Some(AbstractLiteral::Tuple(elements)))
}

fn parse_matrix(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    // Parse elements parse under inner_typechecking_context
    let saved_ctx = ctx.typechecking_context;
    ctx.typechecking_context = ctx.inner_typechecking_context;
    ctx.inner_typechecking_context = TypecheckingContext::Unknown;

    let mut elements = vec![];
    let mut domain: Option<DomainPtr> = None;
    for child in named_children(node) {
        if child.kind() == "arithmetic_expr"
            || child.kind() == "bool_expr"
            || child.kind() == "comparison_expr"
            || child.kind() == "atom"
        {
            let Some(expr) = parse_expression(ctx, child)? else {
                ctx.inner_typechecking_context = ctx.typechecking_context;
                ctx.typechecking_context = saved_ctx;
                return Ok(None);
            };
            elements.push(expr);
        } else {
            let Some(parsed_domain) = parse_domain(ctx, child)? else {
                ctx.inner_typechecking_context = ctx.typechecking_context;
                ctx.typechecking_context = saved_ctx;
                return Ok(None);
            };
            domain = Some(parsed_domain);
        }
    }
    if domain.is_none() {
        let count = elements.len() as i32;
        domain = Some(domain_int!(1..count));
    }

    ctx.inner_typechecking_context = ctx.typechecking_context;
    ctx.typechecking_context = saved_ctx;
    Ok(Some(AbstractLiteral::Matrix(elements, domain.unwrap())))
}

fn parse_set_literal(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    // Parse elements parse under inner_typechecking_context, not typechecking_context.
    let saved_ctx = ctx.typechecking_context;
    ctx.typechecking_context = ctx.inner_typechecking_context;
    ctx.inner_typechecking_context = TypecheckingContext::Unknown;

    let mut elements = Vec::new();
    for child in named_children(node) {
        let Some(expr) = parse_expression(ctx, child)? else {
            ctx.inner_typechecking_context = ctx.typechecking_context;
            ctx.typechecking_context = saved_ctx;
            return Ok(None);
        };
        elements.push(expr);
    }

    ctx.inner_typechecking_context = ctx.typechecking_context;
    ctx.typechecking_context = saved_ctx;
    Ok(Some(AbstractLiteral::Set(elements)))
}

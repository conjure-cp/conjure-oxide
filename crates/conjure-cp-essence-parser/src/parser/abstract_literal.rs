use crate::errors::{FatalParseError, RecoverableParseError};
use crate::expression::parse_expression_with_context;
use crate::field;
use crate::parser::atom::ExpressionContext;
use crate::parser::ParseContext;
use crate::parser::domain::parse_domain;
use crate::util::named_children;
use conjure_cp_core::ast::{AbstractLiteral, DomainPtr, Expression};
use conjure_cp_core::{domain_int, range};
use tree_sitter::Node;

pub fn parse_abstract(
    ctx: &mut ParseContext,
    node: &Node,
    context: ExpressionContext,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    match node.kind() {
        "record" => parse_record(ctx, node, context),
        "tuple" => parse_tuple(ctx, node, context),
        "matrix" => parse_matrix(ctx, node, context),
        "set_literal" => parse_set_literal(ctx, node, context),
        _ => Err(FatalParseError::internal_error(
            format!("Expected abstract literal, got: {}", node.kind()),
            Some(node.range()),
        )),
    }
}

fn parse_record(
    ctx: &mut ParseContext,
    node: &Node,
    context: ExpressionContext,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    let mut values = Vec::new();
    for child in node.children_by_field_name("name_value_pair", &mut node.walk()) {
        let name_node = field!(child, "name");
        let name_str = &ctx.source_code[name_node.start_byte()..name_node.end_byte()];
        let name = conjure_cp_core::ast::Name::user(name_str);

        let Some(value) = parse_expression_with_context(ctx, field!(child, "value"), context)? else {
            return Ok(None);
        };
        values.push(conjure_cp_core::ast::records::RecordValue { name, value });
    }
    Ok(Some(AbstractLiteral::Record(values)))
}

fn parse_tuple(
    ctx: &mut ParseContext,
    node: &Node,
    context: ExpressionContext,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    let mut elements = Vec::new();
    for child in named_children(node) {
        let Some(expr) = parse_expression_with_context(ctx, child, context)? else {
            return Ok(None);
        };
        elements.push(expr);
    }
    Ok(Some(AbstractLiteral::Tuple(elements)))
}

fn parse_matrix(
    ctx: &mut ParseContext,
    node: &Node,
    context: ExpressionContext,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    let mut elements = vec![];
    let mut domain: Option<DomainPtr> = None;
    for child in named_children(node) {
        if child.kind() == "arithmetic_expr"
            || child.kind() == "bool_expr"
            || child.kind() == "comparison_expr"
            || child.kind() == "atom"
        {
            let Some(expr) = parse_expression_with_context(ctx, child, context)? else {
                return Ok(None);
            };
            elements.push(expr);
        } else {
            let Some(parsed_domain) = parse_domain(ctx, child)? else {
                return Ok(None);
            };
            domain = Some(parsed_domain);
        }
    }
    if domain.is_none() {
        let count = elements.len() as i32;
        domain = Some(domain_int!(1..count));
    }

    Ok(Some(AbstractLiteral::Matrix(elements, domain.unwrap())))
}

fn parse_set_literal(
    ctx: &mut ParseContext,
    node: &Node,
    context: ExpressionContext,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    let mut elements = Vec::new();
    for child in named_children(node) {
        let Some(expr) = parse_expression_with_context(ctx, child, context)? else {
            return Ok(None);
        };
        elements.push(expr);
    }
    Ok(Some(AbstractLiteral::Set(elements)))
}

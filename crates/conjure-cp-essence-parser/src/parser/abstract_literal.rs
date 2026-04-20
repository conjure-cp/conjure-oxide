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
    // If we're in a set context, we can only parse set literals, so add an error if we see any other kind of abstract literal
    if ctx.typechecking_context == TypecheckingContext::Set && node.kind() != "set_literal" {
        ctx.record_error(RecoverableParseError::new(
            format!(
                "Type error: {}\n\tExpected: set\n\tGot: {}",
                ctx.source_code[node.start_byte()..node.end_byte()].trim(),
                node.kind()
            ),
            Some(node.range()),
        ));
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
        let Some(value) = parse_expression(ctx, value_node)? else {
            return Ok(None);
        };
        values.push(conjure_cp_core::ast::records::FieldValue { name, value });
    }
    Ok(Some(AbstractLiteral::Record(values)))
}

fn parse_tuple(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    let mut elements = Vec::new();
    for child in named_children(node) {
        let Some(expr) = parse_expression(ctx, child)? else {
            return Ok(None);
        };
        elements.push(expr);
    }
    Ok(Some(AbstractLiteral::Tuple(elements)))
}

fn parse_matrix(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    let mut elements = vec![];
    let mut domain: Option<DomainPtr> = None;
    for child in named_children(node) {
        if child.kind() == "arithmetic_expr"
            || child.kind() == "bool_expr"
            || child.kind() == "comparison_expr"
            || child.kind() == "atom"
        {
            let Some(expr) = parse_expression(ctx, child)? else {
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
) -> Result<Option<AbstractLiteral<Expression>>, FatalParseError> {
    let mut elements = Vec::new();
    for child in named_children(node) {
        ctx.typechecking_context = TypecheckingContext::Unknown;
        let Some(expr) = parse_expression(ctx, child)? else {
            return Ok(None);
        };
        elements.push(expr);
    }
    Ok(Some(AbstractLiteral::Set(elements)))
}

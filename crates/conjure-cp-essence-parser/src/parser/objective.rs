use crate::errors::FatalParseError;
use crate::expression::parse_expression;
use crate::field;
use crate::parser::ParseContext;
use crate::util::TypecheckingContext;
use conjure_cp_core::ast::{Objective, OptimiseDirection};
use tree_sitter::Node;

pub fn parse_objective_statement(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Objective>, FatalParseError> {
    let direction_node = field!(node, "direction");
    let direction_str = &ctx.source_code[direction_node.start_byte()..direction_node.end_byte()];
    let direction = match direction_str {
        "minimising" => OptimiseDirection::Minimising,
        "maximising" => OptimiseDirection::Maximising,
        _ => {
            return Err(FatalParseError::internal_error(
                format!("Unexpected objective direction: '{direction_str}'"),
                Some(direction_node.range()),
            ));
        }
    };

    let Some(expression_node) = field!(recover, ctx, node, "expression") else {
        return Ok(None);
    };

    let saved_context = ctx.typechecking_context;
    let saved_inner_context = ctx.inner_typechecking_context;
    ctx.typechecking_context = TypecheckingContext::Unknown;
    ctx.inner_typechecking_context = TypecheckingContext::Unknown;

    let parsed = parse_expression(ctx, expression_node);

    ctx.typechecking_context = saved_context;
    ctx.inner_typechecking_context = saved_inner_context;

    let Some(expression) = parsed? else {
        return Ok(None);
    };

    Ok(Some(Objective {
        direction,
        expression,
    }))
}

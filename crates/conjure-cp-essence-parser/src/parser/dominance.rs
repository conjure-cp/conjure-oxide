use crate::errors::{FatalParseError, RecoverableParseError};
use crate::expression::parse_expression;
use crate::field;
use crate::parser::ParseContext;
use crate::util::{TypecheckingContext, named_children};
use conjure_cp_core::ast::{
    Atom, DeclarationKind, Expression, Metadata, Moo, ReturnType, Typeable,
};
use conjure_cp_core::into_matrix_expr;
use tree_sitter::Node;
use uniplate::Uniplate;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ParetoDirection {
    Minimising,
    Maximising,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReferenceRewriteAction {
    LeaveAsIs,
    ExpandValueLetting,
    WrapInFromSolution,
}

pub fn parse_dominance_relation(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    if ctx.root.kind() == "dominance_relation" {
        ctx.record_error(RecoverableParseError::new(
            "Nested dominance relations are not allowed".to_string(),
            Some(node.range()),
        ));
        return Ok(None);
    }

    let Some(inner_node) = field!(recover, ctx, node, "expression") else {
        return Ok(None);
    };

    // Create a nested context so downstream parsing knows it is inside a dominance relation.
    let mut inner_ctx = ParseContext {
        source_code: ctx.source_code,
        root: node,
        symbols: ctx.symbols.clone(),
        errors: ctx.errors,
        source_map: &mut *ctx.source_map,
        decl_spans: ctx.decl_spans,
        typechecking_context: TypecheckingContext::Unknown,
        inner_typechecking_context: TypecheckingContext::Unknown,
    };

    let Some(inner) = parse_expression(&mut inner_ctx, inner_node)? else {
        return Ok(None);
    };

    Ok(Some(Expression::DominanceRelation(
        Metadata::new(),
        Moo::new(inner),
    )))
}

pub fn parse_pareto_expression(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    if ctx.root.kind() != "dominance_relation" {
        ctx.record_error(RecoverableParseError::new(
            "pareto(...) only allowed inside dominance relations".to_string(),
            Some(node.range()),
        ));
        return Ok(None);
    }

    let mut non_worsening = Vec::new();
    let mut strict_improvements = Vec::new();
    let components = field!(node, "components");

    if components.kind() != "pareto_items" {
        return Err(FatalParseError::internal_error(
            format!("Unexpected pareto component list: '{}'", components.kind()),
            Some(components.range()),
        ));
    }

    for item_node in named_children(&components) {
        let direction_node = field!(item_node, "direction");
        let direction_str =
            &ctx.source_code[direction_node.start_byte()..direction_node.end_byte()];
        let direction = match direction_str {
            "minimising" => ParetoDirection::Minimising,
            "maximising" => ParetoDirection::Maximising,
            _ => {
                return Err(FatalParseError::internal_error(
                    format!("Unexpected pareto direction: '{direction_str}'"),
                    Some(direction_node.range()),
                ));
            }
        };

        let component_node = field!(item_node, "expression");
        let Some(component_expr) = parse_pareto_component(ctx, &component_node)? else {
            return Ok(None);
        };
        let Some((non_worse, strict)) =
            build_pareto_constraints(ctx, &component_node, component_expr, direction)
        else {
            return Ok(None);
        };
        non_worsening.push(non_worse);
        strict_improvements.push(strict);
    }

    let mut conjuncts = non_worsening;
    conjuncts.push(combine_with_and_or(strict_improvements, true));

    Ok(Some(combine_with_and_or(conjuncts, false)))
}

fn parse_pareto_component(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    let saved_context = ctx.typechecking_context;
    let saved_inner_context = ctx.inner_typechecking_context;
    ctx.typechecking_context = TypecheckingContext::Unknown;
    ctx.inner_typechecking_context = TypecheckingContext::Unknown;

    let parsed = parse_expression(ctx, *node);

    ctx.typechecking_context = saved_context;
    ctx.inner_typechecking_context = saved_inner_context;
    parsed
}

fn build_pareto_constraints(
    ctx: &mut ParseContext,
    node: &Node,
    component: Expression,
    direction: ParetoDirection,
) -> Option<(Expression, Expression)> {
    if component
        .universe()
        .iter()
        .any(|expr| matches!(expr, Expression::FromSolution(_, _)))
    {
        ctx.record_error(RecoverableParseError::new(
            "pareto(...) components cannot contain fromSolution(...) explicitly".to_string(),
            Some(node.range()),
        ));
        return None;
    }

    let current = expand_value_lettings(&component);
    let previous = lift_to_previous_solution(&current);

    match current.return_type() {
        ReturnType::Int => Some(match direction {
            ParetoDirection::Minimising => (
                Expression::Leq(
                    Metadata::new(),
                    Moo::new(current.clone()),
                    Moo::new(previous.clone()),
                ),
                Expression::Lt(Metadata::new(), Moo::new(current), Moo::new(previous)),
            ),
            ParetoDirection::Maximising => (
                Expression::Geq(
                    Metadata::new(),
                    Moo::new(current.clone()),
                    Moo::new(previous.clone()),
                ),
                Expression::Gt(Metadata::new(), Moo::new(current), Moo::new(previous)),
            ),
        }),
        ReturnType::Bool => Some(match direction {
            ParetoDirection::Minimising => (
                Expression::Imply(
                    Metadata::new(),
                    Moo::new(current.clone()),
                    Moo::new(previous.clone()),
                ),
                combine_with_and_or(
                    vec![
                        Expression::Not(Metadata::new(), Moo::new(current)),
                        previous,
                    ],
                    false,
                ),
            ),
            ParetoDirection::Maximising => (
                Expression::Imply(
                    Metadata::new(),
                    Moo::new(previous.clone()),
                    Moo::new(current.clone()),
                ),
                combine_with_and_or(
                    vec![
                        current,
                        Expression::Not(Metadata::new(), Moo::new(previous)),
                    ],
                    false,
                ),
            ),
        }),
        found => {
            ctx.record_error(RecoverableParseError::new(
                format!(
                    "pareto(...) only supports int or bool components, found '{}'",
                    found
                ),
                Some(node.range()),
            ));
            None
        }
    }
}

fn expand_value_lettings(expr: &Expression) -> Expression {
    rewrite_references(expr, false)
}

fn lift_to_previous_solution(expr: &Expression) -> Expression {
    rewrite_references(expr, true)
}

fn rewrite_references(expr: &Expression, to_previous_solution: bool) -> Expression {
    let mut lifted = expr.clone();

    loop {
        let next = lifted.rewrite(&|subexpr| match subexpr {
            Expression::Atomic(_, Atom::Reference(ref reference)) => {
                let action = {
                    let kind = reference.ptr.kind();
                    match &*kind {
                        DeclarationKind::Find(_) if to_previous_solution => {
                            ReferenceRewriteAction::WrapInFromSolution
                        }
                        DeclarationKind::Find(_) => ReferenceRewriteAction::LeaveAsIs,
                        DeclarationKind::ValueLetting(_, _)
                        | DeclarationKind::TemporaryValueLetting(_) => {
                            ReferenceRewriteAction::ExpandValueLetting
                        }
                        DeclarationKind::Given(_)
                        | DeclarationKind::Quantified(_)
                        | DeclarationKind::QuantifiedExpr(_)
                        | DeclarationKind::DomainLetting(_)
                        | DeclarationKind::Field(_)
                        | _ => ReferenceRewriteAction::LeaveAsIs,
                    }
                };

                match action {
                    ReferenceRewriteAction::LeaveAsIs => Some(subexpr),
                    ReferenceRewriteAction::ExpandValueLetting => reference.resolve_expression(),
                    ReferenceRewriteAction::WrapInFromSolution => Some(Expression::FromSolution(
                        Metadata::new(),
                        Moo::new(Atom::Reference(reference.clone())),
                    )),
                }
            }
            _ => Some(subexpr),
        });

        if next == lifted {
            return lifted;
        }

        lifted = next;
    }
}

fn combine_with_and_or(exprs: Vec<Expression>, is_or: bool) -> Expression {
    match exprs.len() {
        0 => {
            if is_or {
                Expression::Or(Metadata::new(), Moo::new(into_matrix_expr![exprs]))
            } else {
                Expression::And(Metadata::new(), Moo::new(into_matrix_expr![exprs]))
            }
        }
        1 => match exprs.into_iter().next() {
            Some(expr) => expr,
            None => unreachable!("vector length already checked"),
        },
        _ => {
            if is_or {
                Expression::Or(Metadata::new(), Moo::new(into_matrix_expr![exprs]))
            } else {
                Expression::And(Metadata::new(), Moo::new(into_matrix_expr![exprs]))
            }
        }
    }
}

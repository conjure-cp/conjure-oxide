use crate::diagnostics::diagnostics_api::SymbolKind;
use crate::diagnostics::source_map::{HoverInfo, span_with_hover};
use crate::errors::{FatalParseError, RecoverableParseError};
use crate::parser::ParseContext;
use crate::parser::atom::parse_atom;
use crate::parser::comprehension::parse_quantifier_or_aggregate_expr;
use crate::util::TypecheckingContext;
use crate::util::named_children;
use crate::{field, named_child};
use conjure_cp_core::ast::{Atom, DeclarationKind, ReturnType, Typeable};
use conjure_cp_core::ast::{Expression, GroundDomain, Metadata, Moo};
use conjure_cp_core::into_matrix_expr;
use conjure_cp_core::{domain_int, matrix_expr, range};
use tree_sitter::Node;
use uniplate::Uniplate;

pub fn parse_expression(
    ctx: &mut ParseContext,
    node: Node,
) -> Result<Option<Expression>, FatalParseError> {
    match node.kind() {
        "atom" => parse_atom(ctx, &node),
        "bool_expr" => {
            if ctx.typechecking_context == TypecheckingContext::Arithmetic {
                ctx.record_error(RecoverableParseError::new(
                    format!(
                        "Type error: {}\n\tExepected: int\n\tGot: boolean expression",
                        ctx.source_code[node.start_byte()..node.end_byte()].to_string()
                    ),
                    Some(node.range()),
                ));
                return Ok(None);
            }
            parse_boolean_expression(ctx, &node)
        }
        "arithmetic_expr" => {
            if ctx.typechecking_context == TypecheckingContext::Boolean {
                ctx.record_error(RecoverableParseError::new(
                    format!(
                        "Type error: {}\n\tExepected: bool\n\tGot: arithmetic expression",
                        ctx.source_code[node.start_byte()..node.end_byte()].to_string()
                    ),
                    Some(node.range()),
                ));
                return Ok(None);
            }
            parse_arithmetic_expression(ctx, &node)
        }
        "comparison_expr" => {
            if ctx.typechecking_context == TypecheckingContext::Arithmetic {
                ctx.record_error(RecoverableParseError::new(
                    format!(
                        "Type error: {}\n\tExepected: int\n\tGot: comparison expression",
                        ctx.source_code[node.start_byte()..node.end_byte()].to_string()
                    ),
                    Some(node.range()),
                ));
                return Ok(None);
            }
            parse_comparison_expression(ctx, &node)
        }
        "all_diff_comparison" => {
            if ctx.typechecking_context == TypecheckingContext::Arithmetic {
                ctx.record_error(RecoverableParseError::new(
                    format!("Type error: {}\n\tExepected: arithmetic expression\n\tFound: comparison expression", ctx.source_code[node.start_byte()..node.end_byte()].to_string()),
                    Some(node.range()),
                ));
                return Ok(None);
            }
            ctx.typechecking_context = TypecheckingContext::Unknown;
            parse_all_diff_comparison(ctx, &node)
        }
        "dominance_relation" => parse_dominance_relation(ctx, &node),
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Unexpected expression type: '{}'", node.kind()),
                Some(node.range()),
            ));
            Ok(None)
        }
    }
}

fn parse_dominance_relation(
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

    // NB: In all other cases, we keep the root the same;
    // However, here we create a new context with the new root so downstream functions
    // know we are inside a dominance relation
    let mut inner_ctx = ParseContext {
        source_code: ctx.source_code,
        root: node,
        symbols: ctx.symbols.clone(),
        errors: ctx.errors,
        source_map: &mut *ctx.source_map,
        decl_spans: ctx.decl_spans,
        typechecking_context: ctx.typechecking_context,
        inner_typechecking_context: ctx.inner_typechecking_context,
    };

    let Some(inner) = parse_expression(&mut inner_ctx, inner_node)? else {
        return Ok(None);
    };

    Ok(Some(Expression::DominanceRelation(
        Metadata::new(),
        Moo::new(inner),
    )))
}

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
    ctx.typechecking_context = TypecheckingContext::Unknown;
    let parsed = parse_expression(ctx, *node)?;
    ctx.typechecking_context = saved_context;
    Ok(parsed)
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
                        | DeclarationKind::RecordField(_)
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

fn parse_arithmetic_expression(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    ctx.typechecking_context = TypecheckingContext::Arithmetic;
    ctx.inner_typechecking_context = TypecheckingContext::Unknown;
    let Some(inner) = named_child!(recover, ctx, node) else {
        return Ok(None);
    };
    match inner.kind() {
        "atom" => parse_atom(ctx, &inner),
        "negative_expr" | "abs_value" | "sub_arith_expr" | "factorial_expr" => {
            parse_unary_expression(ctx, &inner)
        }
        "toInt_expr" => {
            // add special handling for toInt, as it is arithmetic but takes a non-arithmetic operand
            ctx.typechecking_context = TypecheckingContext::Unknown;
            parse_unary_expression(ctx, &inner)
        }
        "exponent" | "product_expr" | "sum_expr" => parse_binary_expression(ctx, &inner),
        "list_combining_expr_arith" => {
            // list-combining arithmetic operators accept either set or matrix operands
            ctx.typechecking_context = TypecheckingContext::SetOrMatrix;

            // set inner context to arithmetic to ensure elements of list are arithmetic expressions
            ctx.inner_typechecking_context = TypecheckingContext::Arithmetic;
            parse_list_combining_expression(ctx, &inner)
        }
        "aggregate_expr" => {
            ctx.inner_typechecking_context = TypecheckingContext::Arithmetic;
            parse_quantifier_or_aggregate_expr(ctx, &inner)
        }
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Expected arithmetic expression, found: {}", inner.kind()),
                Some(inner.range()),
            ));
            Ok(None)
        }
    }
}

fn parse_comparison_expression(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    let Some(inner) = named_child!(recover, ctx, node) else {
        return Ok(None);
    };
    match inner.kind() {
        "arithmetic_comparison" => {
            // Arithmetic comparisons require arithmetic operands
            ctx.typechecking_context = TypecheckingContext::Arithmetic;
            parse_binary_expression(ctx, &inner)
        }
        "lex_comparison" => {
            // TODO: check that both operands are comparable collections.
            ctx.typechecking_context = TypecheckingContext::Unknown;
            parse_binary_expression(ctx, &inner)
        }
        "equality_comparison" => {
            // Equality works on any type, typechecking of operands will be handled within parse_binary_expression
            ctx.typechecking_context = TypecheckingContext::Unknown;
            parse_binary_expression(ctx, &inner)
        }
        "set_comparison" => {
            // Set comparisons require set operands (except 'in', which is hadled later)
            ctx.typechecking_context = TypecheckingContext::Set;
            parse_binary_expression(ctx, &inner)
        }
        "all_diff_comparison" => {
            // TODO: check that operand is a matrix.
            ctx.typechecking_context = TypecheckingContext::Unknown;
            parse_all_diff_comparison(ctx, &inner)
        }
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Expected comparison expression, found '{}'", inner.kind()),
                Some(inner.range()),
            ));
            Ok(None)
        }
    }
}

fn parse_boolean_expression(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    ctx.typechecking_context = TypecheckingContext::Boolean;
    ctx.inner_typechecking_context = TypecheckingContext::Unknown;
    let Some(inner) = named_child!(recover, ctx, node) else {
        return Ok(None);
    };
    match inner.kind() {
        "atom" => parse_atom(ctx, &inner),
        "not_expr" | "sub_bool_expr" => parse_unary_expression(ctx, &inner),
        "and_expr" | "or_expr" | "implication" | "iff_expr" => parse_binary_expression(ctx, &inner),
        "list_combining_expr_bool" => {
            // list-combining boolean operators accept either set or matrix operands
            ctx.typechecking_context = TypecheckingContext::SetOrMatrix;

            // set inner context to boolean to ensure elements of list are boolean expressions
            ctx.inner_typechecking_context = TypecheckingContext::Boolean;
            parse_list_combining_expression(ctx, &inner)
        }
        "quantifier_expr" => parse_quantifier_or_aggregate_expr(ctx, &inner),
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Expected boolean expression, found '{}'", inner.kind()),
                Some(inner.range()),
            ));
            Ok(None)
        }
    }
}

fn parse_list_combining_expression(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    let Some(operator_node) = field!(recover, ctx, node, "operator") else {
        return Ok(None);
    };
    let operator_str = &ctx.source_code[operator_node.start_byte()..operator_node.end_byte()];

    let Some(arg_node) = field!(recover, ctx, node, "arg") else {
        return Ok(None);
    };
    // While parsing inner, the typechecking context is SetOrMatrix
    // The inner context is either Boolean or Arithmetic so the elements of the set/matrix are typechecked correctly.
    let Some(inner) = parse_atom(ctx, &arg_node)? else {
        return Ok(None);
    };

    match operator_str {
        "and" => Ok(Some(Expression::And(Metadata::new(), Moo::new(inner)))),
        "or" => Ok(Some(Expression::Or(Metadata::new(), Moo::new(inner)))),
        "sum" => Ok(Some(Expression::Sum(Metadata::new(), Moo::new(inner)))),
        "product" => Ok(Some(Expression::Product(Metadata::new(), Moo::new(inner)))),
        "min" => Ok(Some(Expression::Min(Metadata::new(), Moo::new(inner)))),
        "max" => Ok(Some(Expression::Max(Metadata::new(), Moo::new(inner)))),
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Invalid operator: '{operator_str}'"),
                Some(operator_node.range()),
            ));
            Ok(None)
        }
    }
}

fn parse_all_diff_comparison(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    let Some(arg_node) = field!(recover, ctx, node, "arg") else {
        return Ok(None);
    };
    let Some(inner) = parse_expression(ctx, arg_node)? else {
        return Ok(None);
    };

    Ok(Some(Expression::AllDiff(Metadata::new(), Moo::new(inner))))
}

fn parse_unary_expression(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    let Some(expr_node) = field!(recover, ctx, node, "expression") else {
        return Ok(None);
    };
    let Some(inner) = parse_expression(ctx, expr_node)? else {
        return Ok(None);
    };
    match node.kind() {
        "negative_expr" => Ok(Some(Expression::Neg(Metadata::new(), Moo::new(inner)))),
        "abs_value" => Ok(Some(Expression::Abs(Metadata::new(), Moo::new(inner)))),
        "not_expr" => Ok(Some(Expression::Not(Metadata::new(), Moo::new(inner)))),
        "toInt_expr" => Ok(Some(Expression::ToInt(Metadata::new(), Moo::new(inner)))),
        "factorial_expr" => Ok(Some(Expression::Factorial(
            Metadata::new(),
            Moo::new(inner),
        ))),
        "sub_bool_expr" | "sub_arith_expr" => Ok(Some(inner)),
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Unrecognised unary operation: '{}'", node.kind()),
                Some(node.range()),
            ));
            Ok(None)
        }
    }
}

pub fn parse_binary_expression(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    let op_node = field!(node, "operator");
    let op_str = &ctx.source_code[op_node.start_byte()..op_node.end_byte()];

    let saved_ctx = ctx.typechecking_context;

    // Special handling for 'in' operator, as the left operand doesn't have to be a set
    if op_str == "in" {
        ctx.typechecking_context = TypecheckingContext::Unknown
    }

    // parse left operand
    let Some(left_node) = field!(recover, ctx, node, "left") else {
        return Ok(None);
    };
    let Some(left) = parse_expression(ctx, left_node)? else {
        return Ok(None);
    };

    // reset context, if needed
    ctx.typechecking_context = saved_ctx;

    // Equality/inequality: enforce right operand to match left operand type when inferable
    if matches!(op_str, "=" | "!=") {
        ctx.typechecking_context = inferred_context_from_expression(&left);
    }

    // parse right operand
    let Some(right_node) = field!(recover, ctx, node, "right") else {
        return Ok(None);
    };
    let Some(right) = parse_expression(ctx, right_node)? else {
        return Ok(None);
    };

    // restore original contexts for parent expression parsing
    ctx.typechecking_context = saved_ctx;

    let mut description = format!("Operator '{op_str}'");
    let expr = match op_str {
        // NB: We are deliberately setting the index domain to 1.., not 1..2.
        // Semantically, this means "a list that can grow/shrink arbitrarily".
        // This is expected by rules which will modify the terms of the sum expression
        // (e.g. by partially evaluating them).
        "+" => Ok(Some(Expression::Sum(
            Metadata::new(),
            Moo::new(matrix_expr![left, right; domain_int!(1..)]),
        ))),
        "-" => Ok(Some(Expression::Minus(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        ))),
        "*" => Ok(Some(Expression::Product(
            Metadata::new(),
            Moo::new(matrix_expr![left, right; domain_int!(1..)]),
        ))),
        "/\\" => Ok(Some(Expression::And(
            Metadata::new(),
            Moo::new(matrix_expr![left, right; domain_int!(1..)]),
        ))),
        "\\/" => Ok(Some(Expression::Or(
            Metadata::new(),
            Moo::new(matrix_expr![left, right; domain_int!(1..)]),
        ))),
        "**" => Ok(Some(Expression::UnsafePow(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        ))),
        "/" => {
            //TODO: add checks for if division is safe or not
            Ok(Some(Expression::UnsafeDiv(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "%" => {
            //TODO: add checks for if mod is safe or not
            Ok(Some(Expression::UnsafeMod(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "=" => Ok(Some(Expression::Eq(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        ))),
        "!=" => Ok(Some(Expression::Neq(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        ))),
        "<=" => Ok(Some(Expression::Leq(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        ))),
        ">=" => Ok(Some(Expression::Geq(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        ))),
        "<" => Ok(Some(Expression::Lt(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        ))),
        ">" => Ok(Some(Expression::Gt(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        ))),
        "->" => Ok(Some(Expression::Imply(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        ))),
        "<->" => Ok(Some(Expression::Iff(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        ))),
        "<lex" => Ok(Some(Expression::LexLt(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        ))),
        ">lex" => Ok(Some(Expression::LexGt(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        ))),
        "<=lex" => Ok(Some(Expression::LexLeq(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        ))),
        ">=lex" => Ok(Some(Expression::LexGeq(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        ))),
        "in" => Ok(Some(Expression::In(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        ))),
        "subset" => Ok(Some(Expression::Subset(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        ))),
        "subsetEq" => Ok(Some(Expression::SubsetEq(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        ))),
        "supset" => Ok(Some(Expression::Supset(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        ))),
        "supsetEq" => Ok(Some(Expression::SupsetEq(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        ))),
        "union" => {
            description = "set union: combines the elements from both operands".to_string();
            Ok(Some(Expression::Union(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "intersect" => {
            description =
                "set intersection: keeps only elements common to both operands".to_string();
            Ok(Some(Expression::Intersect(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Invalid operator: '{op_str}'"),
                Some(op_node.range()),
            ));
            Ok(None)
        }
    };

    if expr.is_ok() {
        let hover = HoverInfo {
            description,
            kind: Some(SymbolKind::Function),
            ty: None,
            decl_span: None,
        };
        span_with_hover(&op_node, ctx.source_code, ctx.source_map, hover);
    }

    expr
}

fn inferred_context_from_expression(expr: &Expression) -> TypecheckingContext {
    // TODO: typechecking for index/slice expressions
    if matches!(
        expr,
        Expression::UnsafeIndex(_, _, _) | Expression::UnsafeSlice(_, _, _)
    ) {
        return TypecheckingContext::Unknown;
    }

    let Some(domain) = expr.domain_of() else {
        return TypecheckingContext::Unknown;
    };
    let Some(ground) = domain.resolve() else {
        return TypecheckingContext::Unknown;
    };

    match ground.as_ref() {
        GroundDomain::Bool => TypecheckingContext::Boolean,
        GroundDomain::Int(_) => TypecheckingContext::Arithmetic,
        GroundDomain::Set(_, _) => TypecheckingContext::Set,
        GroundDomain::MSet(_, _) => TypecheckingContext::MSet,
        GroundDomain::Matrix(_, _) => TypecheckingContext::Matrix,
        GroundDomain::Tuple(_) => TypecheckingContext::Tuple,
        GroundDomain::Record(_) => TypecheckingContext::Record,
        GroundDomain::Function(_, _, _) | GroundDomain::Empty(_) => TypecheckingContext::Unknown,
    }
}

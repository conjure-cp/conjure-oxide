use crate::diagnostics::diagnostics_api::SymbolKind;
use crate::errors::{FatalParseError, RecoverableParseError};
use crate::parser::ParseContext;
use crate::parser::atom::parse_atom;
use crate::parser::comprehension::parse_quantifier_or_aggregate_expr;
use crate::util::TypecheckingContext;
use crate::util::named_children;
use crate::{child, field, named_child};
use conjure_cp_core::ast::{Atom, DeclarationKind, ReturnType, Typeable};
use conjure_cp_core::ast::{Expression, Metadata, Moo};
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
        "bool_expr" => parse_boolean_expression(ctx, &node),
        "arithmetic_expr" => parse_arithmetic_expression(ctx, &node),
        "comparison_expr" => parse_comparison_expression(ctx, &node),
        "dominance_relation" => parse_dominance_relation(ctx, &node),
        "all_diff_comparison" => parse_all_diff_comparison(ctx, &node),
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
        "list_combining_expr_arith" => parse_list_combining_expression(ctx, &inner),
        "aggregate_expr" => parse_quantifier_or_aggregate_expr(ctx, &inner),
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
            // Equality works on any type
            // TODO: add type checking to ensure both sides have the same type
            ctx.typechecking_context = TypecheckingContext::Unknown;
            parse_binary_expression(ctx, &inner)
        }
        "set_comparison" => {
            // Set comparisons require set operands (no specific type checking for now)
            // TODO: add typechecking for sets
            ctx.typechecking_context = TypecheckingContext::Unknown;
            parse_binary_expression(ctx, &inner)
        }
        "all_diff_comparison" => {
            // TODO: check that operand is a collection with compatible element type.
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
    let Some(inner) = named_child!(recover, ctx, node) else {
        return Ok(None);
    };
    match inner.kind() {
        "atom" => parse_atom(ctx, &inner),
        "not_expr" | "sub_bool_expr" => parse_unary_expression(ctx, &inner),
        "and_expr" | "or_expr" | "implication" | "iff_expr" => parse_binary_expression(ctx, &inner),
        "list_combining_expr_bool" => parse_list_combining_expression(ctx, &inner),
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
    let Some(inner) = parse_atom(ctx, &arg_node)? else {
        return Ok(None);
    };

    let mut doc_name = "";
    let expr = match operator_str {
        "and" => Ok(Some(Expression::And(Metadata::new(), Moo::new(inner)))),
        "or" => Ok(Some(Expression::Or(Metadata::new(), Moo::new(inner)))),
        "sum" => Ok(Some(Expression::Sum(Metadata::new(), Moo::new(inner)))),
        "product" => Ok(Some(Expression::Product(Metadata::new(), Moo::new(inner)))),
        "min" => {
            doc_name = "min";
            Ok(Some(Expression::Min(Metadata::new(), Moo::new(inner))))
        }
        "max" => {
            doc_name = "max";
            Ok(Some(Expression::Max(Metadata::new(), Moo::new(inner))))
        }
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Invalid operator: '{operator_str}'"),
                Some(operator_node.range()),
            ));
            Ok(None)
        }
    };

    if expr.is_ok() {
        ctx.add_span_and_doc_hover(
            &operator_node,
            doc_name, // using the operator string as the doc key, which should work for all except "and" and "or"
            format!("Operator '{operator_str}'").as_str(),
            SymbolKind::Function,
            None,
            None,
        );
    }

    expr
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

    let all_diff_keyword_node = child!(node, 0, "allDiff");
    ctx.add_span_and_doc_hover(
        &all_diff_keyword_node,
        "allDiff",
        "allDiff comparison expression",
        SymbolKind::Function,
        None,
        None,
    );
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
        "toInt_expr" => {
            let to_int_keyword_node = child!(node, 0, "toInt");
            ctx.add_span_and_doc_hover(
                &to_int_keyword_node,
                "toInt",
                "toInt type conversion function",
                SymbolKind::Function,
                None,
                None,
            );
            Ok(Some(Expression::ToInt(Metadata::new(), Moo::new(inner))))
        }
        "factorial_expr" => {
            // looking for the operator node (either '!' at the end or 'factorial' at the start) to add hover info
            if let Some(op_node) = (0..node.child_count())
                .filter_map(|i| node.child(i.try_into().unwrap()))
                .find(|c| matches!(c.kind(), "!" | "factorial"))
            {
                ctx.add_span_and_doc_hover(
                    &op_node,
                    "post_factorial",
                    "Factorial operator/function",
                    SymbolKind::Function,
                    None,
                    None,
                );
            }

            Ok(Some(Expression::Factorial(
                Metadata::new(),
                Moo::new(inner),
            )))
        }
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
    let Some(left_node) = field!(recover, ctx, node, "left") else {
        return Ok(None);
    };
    let Some(left) = parse_expression(ctx, left_node)? else {
        return Ok(None);
    };
    let Some(right_node) = field!(recover, ctx, node, "right") else {
        return Ok(None);
    };
    let Some(right) = parse_expression(ctx, right_node)? else {
        return Ok(None);
    };

    let Some(op_node) = field!(recover, ctx, node, "operator") else {
        return Ok(None);
    };
    let op_str = &ctx.source_code[op_node.start_byte()..op_node.end_byte()];

    let mut fallback_descr = format!("Operator '{op_str}'");
    let mut doc_name = "";
    let expr = match op_str {
        // NB: We are deliberately setting the index domain to 1.., not 1..2.
        // Semantically, this means "a list that can grow/shrink arbitrarily".
        // This is expected by rules which will modify the terms of the sum expression
        // (e.g. by partially evaluating them).
        "+" => {
            doc_name = "L_Plus";
            Ok(Some(Expression::Sum(
                Metadata::new(),
                Moo::new(matrix_expr![left, right; domain_int!(1..)]),
            )))
        }
        "-" => {
            doc_name = "L_Minus";
            Ok(Some(Expression::Minus(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "*" => {
            doc_name = "L_Times";
            Ok(Some(Expression::Product(
                Metadata::new(),
                Moo::new(matrix_expr![left, right; domain_int!(1..)]),
            )))
        }
        "/\\" => {
            doc_name = "and";
            Ok(Some(Expression::And(
                Metadata::new(),
                Moo::new(matrix_expr![left, right; domain_int!(1..)]),
            )))
        }
        "\\/" => {
            // No documentation for or in Bits
            fallback_descr = "Disjunction (logical or) operator. Returns true if at least one of the operands is true.".to_string();
            Ok(Some(Expression::Or(
                Metadata::new(),
                Moo::new(matrix_expr![left, right; domain_int!(1..)]),
            )))
        }
        "**" => {
            doc_name = "L_Pow";
            Ok(Some(Expression::UnsafePow(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "/" => {
            //TODO: add checks for if division is safe or not
            doc_name = "L_Div";
            Ok(Some(Expression::UnsafeDiv(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "%" => {
            //TODO: add checks for if mod is safe or not
            doc_name = "L_Mod";
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

        "->" => {
            fallback_descr = "Implication operator".to_string();
            Ok(Some(Expression::Imply(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "<->" => {
            fallback_descr = "Biconditional operator (if and only if)".to_string();
            Ok(Some(Expression::Iff(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "<lex" => {
            fallback_descr = "Lexicographic less-than comparison".to_string();
            Ok(Some(Expression::LexLt(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        ">lex" => {
            fallback_descr = "Lexicographic greater-than comparison".to_string();
            Ok(Some(Expression::LexGt(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "<=lex" => {
            fallback_descr = "Lexicographic less-than-or-equal comparison".to_string();
            Ok(Some(Expression::LexLeq(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        ">=lex" => {
            fallback_descr = "Lexicographic greater-than-or-equal comparison".to_string();
            Ok(Some(Expression::LexGeq(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "in" => {
            doc_name = "L_in";
            Ok(Some(Expression::In(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "subset" => {
            doc_name = "L_subset";
            Ok(Some(Expression::Subset(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "subsetEq" => {
            doc_name = "L_subsetEq";
            Ok(Some(Expression::SubsetEq(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "supset" => {
            doc_name = "L_supset";
            Ok(Some(Expression::Supset(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "supsetEq" => {
            doc_name = "L_supsetEq";
            Ok(Some(Expression::SupsetEq(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "union" => {
            doc_name = "L_union";
            Ok(Some(Expression::Union(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "intersect" => {
            doc_name = "L_intersect";
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
        ctx.add_span_and_doc_hover(
            &op_node,
            doc_name,
            fallback_descr.as_str(),
            SymbolKind::Function,
            None,
            None,
        );
    }

    expr
}

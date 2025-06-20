#![allow(clippy::legacy_numeric_constants)]
use tree_sitter::Node;

use conjure_core::ast::{Atom, Expression, Literal, Name};
use conjure_core::metadata::Metadata;
use conjure_core::{into_matrix_expr, matrix_expr};

use crate::errors::EssenceParseError;

use super::util::named_children;

/// Parse an Essence expression into its Conjure AST representation.
pub fn parse_expression(
    constraint: Node,
    source_code: &str,
    root: &Node,
) -> Result<Expression, EssenceParseError> {
    // TODO (gskorokhod) - Factor this further (make match arms into separate functions, extract common logic)
    match constraint.kind() {
        "constraint" | "expression" | "boolean_expr" | "comparison_expr" | "arithmetic_expr"
        | "primary_expr" | "sub_expr" => child_expr(constraint, source_code, root),
        "not_expr" => Ok(Expression::Not(
            Metadata::new(),
            Box::new(child_expr(constraint, source_code, root)?),
        )),
        "abs_value" => Ok(Expression::Abs(
            Metadata::new(),
            Box::new(child_expr(constraint, source_code, root)?),
        )),
        "negative_expr" => Ok(Expression::Neg(
            Metadata::new(),
            Box::new(child_expr(constraint, source_code, root)?),
        )),
        "exponent" | "product_expr" | "sum_expr" | "comparison" | "and_expr" | "or_expr"
        | "implication" => {
            let expr1 = child_expr(constraint, source_code, root)?;
            let op = constraint.child(1).ok_or(format!(
                "Missing operator in expression {}",
                constraint.kind()
            ))?;
            let op_type = &source_code[op.start_byte()..op.end_byte()];
            let expr2_node = constraint.child(2).ok_or(format!(
                "Missing second operand in expression {}",
                constraint.kind()
            ))?;
            let expr2 = parse_expression(expr2_node, source_code, root)?;

            match op_type {
                "**" => Ok(Expression::UnsafePow(
                    Metadata::new(),
                    Box::new(expr1),
                    Box::new(expr2),
                )),
                "+" => Ok(Expression::Sum(
                    Metadata::new(),
                    Box::new(matrix_expr![expr1, expr2]),
                )),
                "-" => Ok(Expression::Minus(
                    Metadata::new(),
                    Box::new(expr1),
                    Box::new(expr2),
                )),
                "*" => Ok(Expression::Product(
                    Metadata::new(),
                    Box::new(matrix_expr![expr1, expr2]),
                )),
                "/" => {
                    //TODO: add checks for if division is safe or not
                    Ok(Expression::UnsafeDiv(
                        Metadata::new(),
                        Box::new(expr1),
                        Box::new(expr2),
                    ))
                }
                "%" => {
                    //TODO: add checks for if mod is safe or not
                    Ok(Expression::UnsafeMod(
                        Metadata::new(),
                        Box::new(expr1),
                        Box::new(expr2),
                    ))
                }
                "=" => Ok(Expression::Eq(
                    Metadata::new(),
                    Box::new(expr1),
                    Box::new(expr2),
                )),
                "!=" => Ok(Expression::Neq(
                    Metadata::new(),
                    Box::new(expr1),
                    Box::new(expr2),
                )),
                "<=" => Ok(Expression::Leq(
                    Metadata::new(),
                    Box::new(expr1),
                    Box::new(expr2),
                )),
                ">=" => Ok(Expression::Geq(
                    Metadata::new(),
                    Box::new(expr1),
                    Box::new(expr2),
                )),
                "<" => Ok(Expression::Lt(
                    Metadata::new(),
                    Box::new(expr1),
                    Box::new(expr2),
                )),
                ">" => Ok(Expression::Gt(
                    Metadata::new(),
                    Box::new(expr1),
                    Box::new(expr2),
                )),
                "/\\" => Ok(Expression::And(
                    Metadata::new(),
                    Box::new(matrix_expr![expr1, expr2]),
                )),
                "\\/" => Ok(Expression::Or(
                    Metadata::new(),
                    Box::new(matrix_expr![expr1, expr2]),
                )),
                "->" => Ok(Expression::Imply(
                    Metadata::new(),
                    Box::new(expr1),
                    Box::new(expr2),
                )),
                _ => Err(format!("Unsupported operator '{}'", op_type).into()),
            }
        }
        "quantifier_expr" => {
            let mut expr_list = Vec::new();
            for expr in named_children(&constraint) {
                expr_list.push(parse_expression(expr, source_code, root)?);
            }
            let quantifier = constraint.child(0).ok_or(format!(
                "Missing quantifier in expression {}",
                constraint.kind()
            ))?;
            let quantifier_type = &source_code[quantifier.start_byte()..quantifier.end_byte()];

            match quantifier_type {
                "and" => Ok(Expression::And(
                    Metadata::new(),
                    Box::new(into_matrix_expr![expr_list]),
                )),
                "or" => Ok(Expression::Or(
                    Metadata::new(),
                    Box::new(into_matrix_expr![expr_list]),
                )),
                "min" => Ok(Expression::Min(
                    Metadata::new(),
                    Box::new(into_matrix_expr![expr_list]),
                )),
                "max" => Ok(Expression::Max(
                    Metadata::new(),
                    Box::new(into_matrix_expr![expr_list]),
                )),
                "sum" => Ok(Expression::Sum(
                    Metadata::new(),
                    Box::new(into_matrix_expr![expr_list]),
                )),
                "allDiff" => Ok(Expression::AllDiff(
                    Metadata::new(),
                    Box::new(into_matrix_expr![expr_list]),
                )),
                _ => Err(format!("Unsupported quantifier {}", constraint.kind()).into()),
            }
        }
        "constant" => {
            let child = constraint.child(0).ok_or(format!(
                "Missing value for constant expression {}",
                constraint.kind()
            ))?;
            match child.kind() {
                "integer" => {
                    let constant_value = &source_code[child.start_byte()..child.end_byte()]
                        .parse::<i32>()
                        .unwrap();
                    Ok(Expression::Atomic(
                        Metadata::new(),
                        Atom::Literal(Literal::Int(*constant_value)),
                    ))
                }
                "TRUE" => Ok(Expression::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Bool(true)),
                )),
                "FALSE" => Ok(Expression::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Bool(false)),
                )),
                _ => Err(format!("Unsupported constant kind: {}", child.kind()).into()),
            }
        }
        "variable" => {
            let variable_name =
                String::from(&source_code[constraint.start_byte()..constraint.end_byte()]);
            Ok(Expression::Atomic(
                Metadata::new(),
                Atom::Reference(Name::User(variable_name)),
            ))
        }
        "from_solution" => match root.kind() {
            "dominance_relation" => {
                let inner = child_expr(constraint, source_code, root)?;
                match inner {
                    Expression::Atomic(_, _) => {
                        Ok(Expression::FromSolution(Metadata::new(), Box::new(inner)))
                    }
                    _ => Err(
                        "Expression inside a `fromSolution()` must be a variable name"
                            .to_string()
                            .into(),
                    ),
                }
            }
            _ => Err(
                "`fromSolution()` is only allowed inside dominance relation definitions"
                    .to_string()
                    .into(),
            ),
        },
        "toInt_expr" => Ok(Expression::ToInt(
            Metadata::new(),
            Box::new(child_expr(constraint, source_code, root)?),
        )),
        _ => Err(format!("{} is not a recognized node kind", constraint.kind()).into()),
    }
}

pub fn child_expr(
    node: Node,
    source_code: &str,
    root: &Node,
) -> Result<Expression, EssenceParseError> {
    match node.named_child(0) {
        Some(child) => parse_expression(child, source_code, root),
        None => Err(format!("Missing node in expression of kind {}", node.kind()).into()),
    }
}

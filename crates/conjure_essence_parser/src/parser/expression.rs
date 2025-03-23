#![allow(clippy::legacy_numeric_constants)]
use tree_sitter::Node;

use conjure_core::ast::{Atom, Expression, Literal, Name};
use conjure_core::metadata::Metadata;
use conjure_core::{into_matrix_expr, matrix_expr};

use super::util::named_children;

/// Parse an Essence expression into its Conjure AST representation.
pub fn parse_expression(constraint: Node, source_code: &str, root: &Node) -> Expression {
    // TODO (gskorokhod) - Factor this further (make match arms into separate functions, extract common logic)
    match constraint.kind() {
        "constraint" | "expression" | "boolean_expr" | "comparison_expr" | "arithmetic_expr"
        | "primary_expr" | "sub_expr" => child_expr(constraint, source_code, root),
        "not_expr" => Expression::Not(
            Metadata::new(),
            Box::new(child_expr(constraint, source_code, root)),
        ),
        "abs_value" => Expression::Abs(
            Metadata::new(),
            Box::new(child_expr(constraint, source_code, root)),
        ),
        "negative_expr" => Expression::Neg(
            Metadata::new(),
            Box::new(child_expr(constraint, source_code, root)),
        ),
        "exponent" | "product_expr" | "sum_expr" | "comparison" | "and_expr" | "or_expr"
        | "implication" => {
            let expr1 = child_expr(constraint, source_code, root);
            let op = constraint.child(1).unwrap_or_else(|| {
                panic!(
                    "Error: missing node in expression of kind {}",
                    constraint.kind()
                )
            });
            let op_type = &source_code[op.start_byte()..op.end_byte()];
            let expr2_node = constraint.child(2).unwrap_or_else(|| {
                panic!(
                    "Error: missing node in expression of kind {}",
                    constraint.kind()
                )
            });
            let expr2 = parse_expression(expr2_node, source_code, root);

            match op_type {
                "**" => Expression::UnsafePow(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                "+" => Expression::Sum(Metadata::new(), vec![expr1, expr2]),
                "-" => Expression::Minus(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                "*" => Expression::Product(Metadata::new(), vec![expr1, expr2]),
                "/" => {
                    //TODO: add checks for if division is safe or not
                    Expression::UnsafeDiv(Metadata::new(), Box::new(expr1), Box::new(expr2))
                }
                "%" => {
                    //TODO: add checks for if mod is safe or not
                    Expression::UnsafeMod(Metadata::new(), Box::new(expr1), Box::new(expr2))
                }
                "=" => Expression::Eq(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                "!=" => Expression::Neq(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                "<=" => Expression::Leq(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                ">=" => Expression::Geq(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                "<" => Expression::Lt(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                ">" => Expression::Gt(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                "/\\" => Expression::And(Metadata::new(), Box::new(matrix_expr![expr1, expr2])),
                "\\/" => Expression::Or(Metadata::new(), Box::new(matrix_expr![expr1, expr2])),
                "->" => Expression::Imply(Metadata::new(), Box::new(expr1), Box::new(expr2)),
                _ => panic!("Error: unsupported operator"),
            }
        }
        "quantifier_expr" => {
            let mut expr_list = Vec::new();
            for expr in named_children(&constraint) {
                expr_list.push(parse_expression(expr, source_code, root));
            }

            let quantifier = constraint.child(0).unwrap_or_else(|| {
                panic!(
                    "Error: missing node in expression of kind {}",
                    constraint.kind()
                )
            });
            let quantifier_type = &source_code[quantifier.start_byte()..quantifier.end_byte()];

            match quantifier_type {
                "and" => Expression::And(Metadata::new(), Box::new(into_matrix_expr![expr_list])),
                "or" => Expression::Or(Metadata::new(), Box::new(into_matrix_expr![expr_list])),
                "min" => Expression::Min(Metadata::new(), Box::new(into_matrix_expr![expr_list])),
                "max" => Expression::Max(Metadata::new(), Box::new(into_matrix_expr![expr_list])),
                "sum" => Expression::Sum(Metadata::new(), expr_list),
                "allDiff" => {
                    Expression::AllDiff(Metadata::new(), Box::new(into_matrix_expr![expr_list]))
                }
                _ => panic!("Error: unsupported quantifier"),
            }
        }
        "constant" => {
            let child = constraint.child(0).unwrap_or_else(|| {
                panic!(
                    "Error: missing node in expression of kind {}",
                    constraint.kind()
                )
            });
            match child.kind() {
                "integer" => {
                    let constant_value = &source_code[child.start_byte()..child.end_byte()]
                        .parse::<i32>()
                        .unwrap();
                    Expression::Atomic(
                        Metadata::new(),
                        Atom::Literal(Literal::Int(*constant_value)),
                    )
                }
                "TRUE" => Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true))),
                "FALSE" => Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false))),
                _ => panic!("Error"),
            }
        }
        "variable" => {
            let variable_name =
                String::from(&source_code[constraint.start_byte()..constraint.end_byte()]);
            Expression::Atomic(
                Metadata::new(),
                Atom::Reference(Name::UserName(variable_name)),
            )
        }
        "from_solution" => match root.kind() {
            "dominance_relation" => {
                let inner = child_expr(constraint, source_code, root);
                match inner {
                    Expression::Atomic(_, _) => {
                        Expression::FromSolution(Metadata::new(), Box::new(inner))
                    }
                    _ => panic!("Expression inside a `fromSolution()` must be a variable name"),
                }
            }
            _ => panic!("`fromSolution()` is only allowed inside dominance relation definitions"),
        },
        "metavar" => {
            let inner = constraint
                .named_child(0)
                .expect("Expected name for meta-variable");
            let name = &source_code[inner.start_byte()..inner.end_byte()];
            Expression::Metavar(Metadata::new(), name.to_string())
        }
        _ => panic!("{} is not a recognized node kind", constraint.kind()),
    }
}

fn child_expr(node: Node, source_code: &str, root: &Node) -> Expression {
    let child = node
        .named_child(0)
        .unwrap_or_else(|| panic!("Error: missing node in expression of kind {}", node.kind()));
    parse_expression(child, source_code, root)
}

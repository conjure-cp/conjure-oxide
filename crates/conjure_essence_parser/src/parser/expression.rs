#![allow(clippy::legacy_numeric_constants)]

use tree_sitter::Node;

use conjure_core::ast::{Atom, Domain, Expression, Literal, Name, Range};
use conjure_core::matrix_expr;
use conjure_core::metadata::Metadata;

use crate::errors::EssenceParseError;

use super::domain::parse_domain;
use super::util::named_children;

/// Parse an Essence expression into its Conjure AST representation.
pub fn parse_expression(
    constraint: Node,
    source_code: &str,
    root: &Node,
) -> Result<Expression, EssenceParseError> {
    // TODO (gskorokhod) - Factor this further (make match arms into separate functions, extract common logic)
    match constraint.kind() {
        "bool_expr" | "arithmetic_expr" | "atom" | "sub_bool_expr" | "sub_arith_expr" => {
            child_expr(constraint, source_code, root)
        }
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
        "exponent" | "product_expr" | "sum_expr" | "comparison_expr" | "and_expr" | "or_expr"
        | "implication" | "iff_expr" => parse_expr_op_expr(constraint, source_code, root),
        "quantifier_expr_bool" | "quantifier_expr_arith" => {
            parse_quatifier_expr(constraint, source_code, root)
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
        "identifier" => {
            let variable_name =
                String::from(&source_code[constraint.start_byte()..constraint.end_byte()]);
            Ok(Expression::Atomic(
                Metadata::new(),
                Atom::Reference(Name::UserName(variable_name)),
            ))
        }
        "tuple_matrix_index_or_slice" => {
            let tuple_or_matrix = child_expr(constraint, source_code, root)?;
            let indices_node = constraint
                .child_by_field_name("indices")
                .ok_or(format!("Missing indices in tuple/matrix expression"))?;
            if indices_node.child_by_field_name("null_index").is_some() {
                let mut indices: Vec<Option<Expression>> = Vec::new();
                for index in named_children(&indices_node) {
                    if index.kind() == "arithmetic_expr" {
                        let index_expr = parse_expression(index, source_code, root)?;
                        indices.push(Some(index_expr));
                    } else {
                        indices.push(None);
                    }
                }
                return Ok(Expression::UnsafeSlice(
                    Metadata::new(),
                    Box::new(tuple_or_matrix),
                    indices,
                ));
            }
            let mut indices: Vec<Expression> = Vec::new();
            for index in named_children(&indices_node) {
                let index_expr = parse_expression(index, source_code, root)?;
                indices.push(index_expr);
            }
            Ok(Expression::UnsafeIndex(
                Metadata::new(),
                Box::new(tuple_or_matrix),
                indices,
            ))
        }
        "tuple" => {
            let mut elements = vec![];
            for element in named_children(&constraint) {
                elements.push(parse_expression(element, source_code, root)?);
            }
            Ok(Expression::AbstractLiteral(
                Metadata::new(),
                conjure_core::ast::AbstractLiteral::Tuple(elements),
            ))
        }
        "matrix" => {
            let mut elements = vec![];
            let mut domain: Option<Domain> = None;
            for element in named_children(&constraint) {
                if element.kind() == "arithmetic_expr" {
                    elements.push(parse_expression(element, source_code, root)?);
                } else {
                    domain = Some(parse_domain(element, source_code));
                }
            }
            if domain.is_none() {
                domain = Some(Domain::IntDomain(vec![Range::Bounded(
                    1,
                    constraint.named_child_count() as i32,
                )]));
            }
            Ok(Expression::AbstractLiteral(
                Metadata::new(),
                conjure_core::ast::AbstractLiteral::Matrix(elements, domain.unwrap()),
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

fn parse_expr_op_expr(
    constraint: Node,
    source_code: &str,
    root: &Node,
) -> Result<Expression, EssenceParseError> {
    let expr1 = child_expr(constraint, source_code, root)?;
    let op = constraint.child_by_field_name("operator").ok_or(format!(
        "Missing operator in expression {}",
        constraint.kind()
    ))?;
    let op_type = &source_code[op.start_byte()..op.end_byte()];
    let expr2_node = constraint.child_by_field_name("right").ok_or(format!(
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
            Box::new(matrix_expr![expr1, expr2;Domain::IntDomain(vec![Range::Bounded(1, 2)])]),
        )),
        "-" => Ok(Expression::Minus(
            Metadata::new(),
            Box::new(expr1),
            Box::new(expr2),
        )),
        "*" => Ok(Expression::Product(Metadata::new(), vec![expr1, expr2])),
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
            Box::new(matrix_expr![expr1, expr2;Domain::IntDomain(vec![Range::Bounded(1, 2)])]),
        )),
        "\\/" => Ok(Expression::Or(
            Metadata::new(),
            Box::new(matrix_expr![expr1, expr2;Domain::IntDomain(vec![Range::Bounded(1, 2)])]),
        )),
        "->" => Ok(Expression::Imply(
            Metadata::new(),
            Box::new(expr1),
            Box::new(expr2),
        )),
        "<->" => Ok(Expression::Iff(
            Metadata::new(),
            Box::new(expr1),
            Box::new(expr2),
        )),
        _ => Err(format!("Unsupported operator '{}'", op_type).into()),
    }
}

fn parse_quatifier_expr(
    constraint: Node,
    source_code: &str,
    root: &Node,
) -> Result<Expression, EssenceParseError> {
    let quantifier = constraint.child_by_field_name("quantifier").ok_or(format!(
        "Missing quantifier in expression {}",
        constraint.kind()
    ))?;
    let quantifier_type = &source_code[quantifier.start_byte()..quantifier.end_byte()];

    // let mut expr_list = Vec::new();
    // for expr in named_children(&constraint) {
    //     expr_list.push(parse_expression(expr, source_code, root)?);
    // }

    let contents = child_expr(constraint, source_code, root)
        .expect("Error parsing contents of quantifier expression");
    // conjures json makes matricies have index domain int(1..n), where n is the number of exprs in the list
    // do that here too
    // let index_domain = Domain::IntDomain(vec![Range::Bounded(1, expr_list.len() as i32)]);
    match quantifier_type {
        "and" => Ok(Expression::And(Metadata::new(), Box::new(contents))),
        "or" => Ok(Expression::Or(Metadata::new(), Box::new(contents))),
        "min" => Ok(Expression::Min(Metadata::new(), Box::new(contents))),
        "max" => Ok(Expression::Max(Metadata::new(), Box::new(contents))),
        "sum" => Ok(Expression::Sum(Metadata::new(), Box::new(contents))),
        "allDiff" => Ok(Expression::AllDiff(Metadata::new(), Box::new(contents))),
        _ => Err(format!("Unsupported quantifier {}", constraint.kind()).into()),
    }
}

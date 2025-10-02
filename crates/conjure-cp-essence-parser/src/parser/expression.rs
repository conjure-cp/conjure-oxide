use tree_sitter::Node;
use ustr::Ustr;

use conjure_cp_core::ast::Metadata;
use conjure_cp_core::ast::{Atom, Expression, Literal, Moo, Name, SymbolTable};
use conjure_cp_core::{into_matrix_expr, matrix_expr};

use crate::errors::EssenceParseError;

use super::util::named_children;

/// Get the i-th named child of a node, or return a syntax error with a message if it doesn't exist.
macro_rules! named_child {
    ($node:ident) => {
        named_child!($node, 0, "Missing sub-expression")
    };
    ($node:ident, $i:literal) => {
        named_child!($node, $i, format!("Missing sub-expression #{}", $i + 1))
    };
    ($node:ident, $i:literal, $msg:expr) => {
        $node
            .named_child($i)
            .ok_or(EssenceParseError::syntax_error(
                format!("{} in expression of kind '{}'", $msg, $node.kind()),
                Some($node.range()),
            ))?
    };
}

/// Get the i-th child of a node, or return a syntax error with a message if it doesn't exist.
macro_rules! child {
    ($node:ident) => {
        child!($node, 0, "Missing sub-expression")
    };
    ($node:ident, $i:literal) => {
        child!($node, $i, format!("Missing sub-expression #{}", $i + 1))
    };
    ($node:ident, $i:literal, $msg:expr) => {
        $node.child($i).ok_or(EssenceParseError::syntax_error(
            format!("{} in expression of kind '{}'", $msg, $node.kind()),
            Some($node.range()),
        ))?
    };
}

/// Parse an Essence expression into its Conjure AST representation.
pub fn parse_expression(
    constraint: Node,
    source_code: &str,
    root: &Node,
    symbols: Option<&SymbolTable>,
) -> Result<Expression, EssenceParseError> {
    // TODO (gskorokhod) - Factor this further (make match arms into separate functions, extract common logic)
    let parse_subexpression = |expr: Node| parse_expression(expr, source_code, root, symbols);

    match constraint.kind() {
        "constraint" | "expression" | "boolean_expr" | "comparison_expr" | "arithmetic_expr"
        | "primary_expr" | "sub_expr" => parse_subexpression(named_child!(constraint)),
        "not_expr" => Ok(Expression::Not(
            Metadata::new(),
            Moo::new(parse_subexpression(named_child!(constraint))?),
        )),
        "abs_value" => Ok(Expression::Abs(
            Metadata::new(),
            Moo::new(parse_subexpression(named_child!(constraint))?),
        )),
        "negative_expr" => Ok(Expression::Neg(
            Metadata::new(),
            Moo::new(parse_subexpression(named_child!(constraint))?),
        )),
        "exponent" | "product_expr" | "sum_expr" | "comparison" | "and_expr" | "or_expr"
        | "implication" => {
            let expr1_node = named_child!(constraint, 0, "Missing first operand");
            let op = child!(constraint, 1, "Missing operator");
            let op_type = &source_code[op.start_byte()..op.end_byte()];
            let expr2_node = child!(constraint, 2, "Missing second operand");

            let expr1 = parse_subexpression(expr1_node)?;
            let expr2 = parse_subexpression(expr2_node)?;

            match op_type {
                "**" => Ok(Expression::UnsafePow(
                    Metadata::new(),
                    Moo::new(expr1),
                    Moo::new(expr2),
                )),
                "+" => Ok(Expression::Sum(
                    Metadata::new(),
                    Moo::new(matrix_expr![expr1, expr2]),
                )),
                "-" => Ok(Expression::Minus(
                    Metadata::new(),
                    Moo::new(expr1),
                    Moo::new(expr2),
                )),
                "*" => Ok(Expression::Product(
                    Metadata::new(),
                    Moo::new(matrix_expr![expr1, expr2]),
                )),
                "/" => {
                    //TODO: add checks for if division is safe or not
                    Ok(Expression::UnsafeDiv(
                        Metadata::new(),
                        Moo::new(expr1),
                        Moo::new(expr2),
                    ))
                }
                "%" => {
                    //TODO: add checks for if mod is safe or not
                    Ok(Expression::UnsafeMod(
                        Metadata::new(),
                        Moo::new(expr1),
                        Moo::new(expr2),
                    ))
                }
                "=" => Ok(Expression::Eq(
                    Metadata::new(),
                    Moo::new(expr1),
                    Moo::new(expr2),
                )),
                "!=" => Ok(Expression::Neq(
                    Metadata::new(),
                    Moo::new(expr1),
                    Moo::new(expr2),
                )),
                "<=" => Ok(Expression::Leq(
                    Metadata::new(),
                    Moo::new(expr1),
                    Moo::new(expr2),
                )),
                ">=" => Ok(Expression::Geq(
                    Metadata::new(),
                    Moo::new(expr1),
                    Moo::new(expr2),
                )),
                "<" => Ok(Expression::Lt(
                    Metadata::new(),
                    Moo::new(expr1),
                    Moo::new(expr2),
                )),
                ">" => Ok(Expression::Gt(
                    Metadata::new(),
                    Moo::new(expr1),
                    Moo::new(expr2),
                )),
                "/\\" => Ok(Expression::And(
                    Metadata::new(),
                    Moo::new(matrix_expr![expr1, expr2]),
                )),
                "\\/" => Ok(Expression::Or(
                    Metadata::new(),
                    Moo::new(matrix_expr![expr1, expr2]),
                )),
                "->" => Ok(Expression::Imply(
                    Metadata::new(),
                    Moo::new(expr1),
                    Moo::new(expr2),
                )),
                _ => Err(EssenceParseError::syntax_error(
                    format!("Unsupported operator '{op_type}'"),
                    Some(op.range()),
                )),
            }
        }
        "quantifier_expr" => {
            let mut expr_list = Vec::new();
            for expr in named_children(&constraint) {
                expr_list.push(parse_subexpression(expr)?);
            }
            let quantifier = child!(constraint, 0, "Missing quantifier");
            let quantifier_type = &source_code[quantifier.start_byte()..quantifier.end_byte()];

            match quantifier_type {
                "and" => Ok(Expression::And(
                    Metadata::new(),
                    Moo::new(into_matrix_expr![expr_list]),
                )),
                "or" => Ok(Expression::Or(
                    Metadata::new(),
                    Moo::new(into_matrix_expr![expr_list]),
                )),
                "min" => Ok(Expression::Min(
                    Metadata::new(),
                    Moo::new(into_matrix_expr![expr_list]),
                )),
                "max" => Ok(Expression::Max(
                    Metadata::new(),
                    Moo::new(into_matrix_expr![expr_list]),
                )),
                "sum" => Ok(Expression::Sum(
                    Metadata::new(),
                    Moo::new(into_matrix_expr![expr_list]),
                )),
                "allDiff" => Ok(Expression::AllDiff(
                    Metadata::new(),
                    Moo::new(into_matrix_expr![expr_list]),
                )),
                _ => Err(EssenceParseError::syntax_error(
                    format!("Unsupported quantifier: '{quantifier_type}'"),
                    Some(quantifier.range()),
                )),
            }
        }
        "constant" => {
            let child = child!(constraint, 0, "Missing sub-expression");
            match child.kind() {
                "integer" => {
                    let constant_value = parse_int(&child, source_code)?;
                    Ok(Expression::Atomic(
                        Metadata::new(),
                        Atom::Literal(Literal::Int(constant_value)),
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
                _ => Err(EssenceParseError::syntax_error(
                    format!("Unsupported constant kind: '{}'", child.kind()),
                    Some(child.range()),
                )),
            }
        }
        "variable" => {
            let variable_name = &source_code[constraint.start_byte()..constraint.end_byte()];
            let name = Name::user(variable_name);

            match symbols {
                Some(symbols) => {
                    // Look up the declaration in the symbol table
                    let declaration = symbols.lookup(&name).ok_or_else(|| {
                        EssenceParseError::syntax_error(
                            format!("Variable '{variable_name}' not found in scope"),
                            Some(constraint.range()),
                        )
                    })?;

                    Ok(Expression::Atomic(
                        Metadata::new(),
                        Atom::Reference(declaration),
                    ))
                }
                None => Err(EssenceParseError::syntax_error(
                    format!(
                        "Found variable: '{variable_name}'. \
                 Did you mean to pass a meta-variable: '&{variable_name}'?\n\
                 Referencing variables by name is not supported because \
                 all references must point to a Declaration, which may not \
                 exist in the current context."
                    ),
                    Some(constraint.range()),
                )),
            }
        }
        "from_solution" => match root.kind() {
            "dominance_relation" => {
                let inner = parse_subexpression(named_child!(constraint))?;
                match inner {
                    Expression::Atomic(_, _) => {
                        Ok(Expression::FromSolution(Metadata::new(), Moo::new(inner)))
                    }
                    _ => Err(EssenceParseError::syntax_error(
                        "Expression inside a `fromSolution()` must be a variable name".to_string(),
                        Some(constraint.range()),
                    )),
                }
            }
            _ => Err(EssenceParseError::syntax_error(
                "`fromSolution()` is only allowed inside dominance relation definitions"
                    .to_string(),
                Some(constraint.range()),
            )),
        },
        "toInt_expr" => Ok(Expression::ToInt(
            Metadata::new(),
            Moo::new(parse_subexpression(named_child!(constraint))?),
        )),
        "metavar" => {
            let text = &source_code[constraint.start_byte()..constraint.end_byte()]
                .trim()
                .strip_prefix("&")
                .ok_or(EssenceParseError::syntax_error(
                    "Meta-variable must start with '&'".to_string(),
                    Some(constraint.range()),
                ))?;
            Ok(Expression::Metavar(
                Metadata::new(),
                Ustr::from(text.trim()),
            ))
        }
        "ERROR" => Err(EssenceParseError::syntax_error(
            format!(
                "'{}' is not a valid expression",
                &source_code[constraint.start_byte()..constraint.end_byte()]
            ),
            Some(constraint.range()),
        )),
        _ => Err(EssenceParseError::syntax_error(
            format!("{} is not a recognized expression kind", constraint.kind()),
            Some(constraint.range()),
        )),
    }
}

fn parse_int(node: &Node, source_code: &str) -> Result<i32, EssenceParseError> {
    let raw_value = &source_code[node.start_byte()..node.end_byte()];
    raw_value.parse::<i32>().map_err(|_e| {
        if raw_value.is_empty() {
            EssenceParseError::syntax_error(
                "Expected an integer here".to_string(),
                Some(node.range()),
            )
        } else {
            EssenceParseError::syntax_error(
                format!("'{raw_value}' is not a valid integer"),
                Some(node.range()),
            )
        }
    })
}

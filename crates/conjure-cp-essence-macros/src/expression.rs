use conjure_cp_essence_parser::errors::EssenceParseError;
use conjure_cp_essence_parser::util::named_children;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;
use tree_sitter::Node;

// TODO (gskorokhod) - This is a lot of code duplication, but I don't see an easy way to avoid it short of a ~visitor pattern~ of some sort.

/// "Sister function" to conjure_cp_essence_parser::::conjure_cp::ast::Expression::parse_::conjure_cp::ast::Expression.
/// Instead of actually constructing the AST, this returns its constructor as a TokenStream.
/// Intended for compile-time parsing inside macros.
pub fn parse_expr_to_ts(
    constraint: Node,
    source_code: &str,
    root: &Node,
) -> Result<TokenStream, EssenceParseError> {
    match constraint.kind() {
        "constraint" | "expression" | "boolean_expr" | "comparison_expr" | "arithmetic_expr"
        | "primary_expr" | "sub_expr" => child_expr_to_ts(constraint, source_code, root),
        "not_expr" => {
            let child = child_expr_to_ts(constraint, source_code, root)?;
            Ok(quote! {::conjure_cp::ast::Expression::Not(
                ::conjure_cp::ast::Metadata::new(),
                ::conjure_cp::ast::Moo::new(#child),
            )})
        }
        "abs_value" => {
            let child = child_expr_to_ts(constraint, source_code, root)?;
            Ok(quote! {
                ::conjure_cp::ast::Expression::Abs(
                ::conjure_cp::ast::Metadata::new(),
                ::conjure_cp::ast::Moo::new(#child),
            )})
        }
        "negative_expr" => {
            let child = child_expr_to_ts(constraint, source_code, root)?;
            Ok(quote! {::conjure_cp::ast::Expression::Neg(
                ::conjure_cp::ast::Metadata::new(),
                ::conjure_cp::ast::Moo::new(#child),
            )})
        }
        "exponent" | "product_expr" | "sum_expr" | "comparison" | "and_expr" | "or_expr"
        | "implication" => {
            let expr1 = child_expr_to_ts(constraint, source_code, root)?;
            let op = constraint.child(1).ok_or(EssenceParseError::syntax_error(
                format!("Missing operator in expression {}", constraint.kind()),
                Some(constraint.range()),
            ))?;
            let op_type = &source_code[op.start_byte()..op.end_byte()];
            let expr2_node = constraint.child(2).ok_or(EssenceParseError::syntax_error(
                format!("Missing second operand in expression {}", constraint.kind()),
                Some(constraint.range()),
            ))?;
            let expr2 = parse_expr_to_ts(expr2_node, source_code, root)?;

            match op_type {
                "**" => Ok(quote! {::conjure_cp::ast::Expression::UnsafePow(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Moo::new(#expr1),
                    ::conjure_cp::ast::Moo::new(#expr2),
                )}),
                "+" => Ok(quote! {::conjure_cp::ast::Expression::Sum(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Moo::new(::conjure_cp::matrix_expr![#expr1, #expr2]),
                )}),
                "-" => Ok(quote! {::conjure_cp::ast::Expression::Minus(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Moo::new(#expr1),
                    ::conjure_cp::ast::Moo::new(#expr2),
                )}),
                "*" => Ok(
                    quote! {::conjure_cp::ast::Expression::Product(::conjure_cp::ast::Metadata::new(), ::conjure_cp::ast::Moo::new(::conjure_cp::matrix_expr![#expr1, #expr2]))},
                ),
                "/" => {
                    //TODO: add checks for if division is safe or not
                    Ok(quote! {::conjure_cp::ast::Expression::UnsafeDiv(
                        ::conjure_cp::ast::Metadata::new(),
                        ::conjure_cp::ast::Moo::new(#expr1),
                        ::conjure_cp::ast::Moo::new(#expr2),
                    )})
                }
                "%" => {
                    //TODO: add checks for if mod is safe or not
                    Ok(quote! {::conjure_cp::ast::Expression::UnsafeMod(
                        ::conjure_cp::ast::Metadata::new(),
                        ::conjure_cp::ast::Moo::new(#expr1),
                        ::conjure_cp::ast::Moo::new(#expr2),
                    )})
                }
                "=" => Ok(quote! {::conjure_cp::ast::Expression::Eq(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Moo::new(#expr1),
                    ::conjure_cp::ast::Moo::new(#expr2),
                )}),
                "!=" => Ok(quote! {::conjure_cp::ast::Expression::Neq(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Moo::new(#expr1),
                    ::conjure_cp::ast::Moo::new(#expr2),
                )}),
                "<=" => Ok(quote! {::conjure_cp::ast::Expression::Leq(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Moo::new(#expr1),
                    ::conjure_cp::ast::Moo::new(#expr2),
                )}),
                ">=" => Ok(quote! {::conjure_cp::ast::Expression::Geq(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Moo::new(#expr1),
                    ::conjure_cp::ast::Moo::new(#expr2),
                )}),
                "<" => Ok(quote! {::conjure_cp::ast::Expression::Lt(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Moo::new(#expr1),
                    ::conjure_cp::ast::Moo::new(#expr2),
                )}),
                ">" => Ok(quote! {::conjure_cp::ast::Expression::Gt(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Moo::new(#expr1),
                    ::conjure_cp::ast::Moo::new(#expr2),
                )}),
                "/\\" => Ok(quote! {::conjure_cp::ast::Expression::And(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Moo::new(::conjure_cp::matrix_expr![#expr1, #expr2]),
                )}),
                "\\/" => Ok(quote! {::conjure_cp::ast::Expression::Or(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Moo::new(::conjure_cp::matrix_expr![#expr1, #expr2]),
                )}),
                "->" => Ok(quote! {::conjure_cp::ast::Expression::Imply(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Moo::new(#expr1),
                    ::conjure_cp::ast::Moo::new(#expr2),
                )}),
                _ => Err(EssenceParseError::syntax_error(
                    format!("Unsupported operator '{op_type}'"),
                    Some(op.range()),
                )),
            }
        }
        "quantifier_expr" => {
            let mut expr_list = Vec::new();
            for expr in named_children(&constraint) {
                expr_list.push(parse_expr_to_ts(expr, source_code, root)?);
            }

            let quantifier = constraint.child(0).ok_or(EssenceParseError::syntax_error(
                format!("Missing quantifier in expression {}", constraint.kind()),
                Some(constraint.range()),
            ))?;
            let quantifier_type = &source_code[quantifier.start_byte()..quantifier.end_byte()];

            match quantifier_type {
                "and" => Ok(quote! {::conjure_cp::ast::Expression::And(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Moo::new(::conjure_cp::matrix_expr![#(#expr_list),*]),
                )}),
                "or" => Ok(quote! {::conjure_cp::ast::Expression::Or(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Moo::new(::conjure_cp::matrix_expr![#(#expr_list),*]),
                )}),
                "min" => Ok(quote! {::conjure_cp::ast::Expression::Min(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Moo::new(::conjure_cp::matrix_expr![#(#expr_list),*]),
                )}),
                "max" => Ok(quote! {::conjure_cp::ast::Expression::Max(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Moo::new(::conjure_cp::matrix_expr![#(#expr_list),*]),
                )}),
                "sum" => Ok(quote! {::conjure_cp::ast::Expression::Sum(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Moo::new(::conjure_cp::matrix_expr![#(#expr_list),*]),
                )}),
                "allDiff" => Ok(quote! {::conjure_cp::ast::Expression::AllDiff(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Moo::new(::conjure_cp::matrix_expr![#(#expr_list),*]),
                )}),
                _ => Err(EssenceParseError::syntax_error(
                    format!("Unsupported quantifier {}", constraint.kind()),
                    Some(quantifier.range()),
                )),
            }
        }
        "constant" => {
            let child = constraint.child(0).ok_or(EssenceParseError::syntax_error(
                format!(
                    "Missing value for constant expression {}",
                    constraint.kind()
                ),
                Some(constraint.range()),
            ))?;
            match child.kind() {
                "integer" => {
                    let raw_value = &source_code[child.start_byte()..child.end_byte()];
                    let constant_value = raw_value.parse::<i32>().map_err(|_e| {
                        if raw_value.is_empty() {
                            EssenceParseError::syntax_error(
                                "expected an integer here".to_string(),
                                Some(child.range()),
                            )
                        } else {
                            EssenceParseError::syntax_error(
                                format!("'{raw_value}' is not a valid integer"),
                                Some(child.range()),
                            )
                        }
                    })?;
                    Ok(quote! {::conjure_cp::ast::Expression::Atomic(
                        ::conjure_cp::ast::Metadata::new(),
                        ::conjure_cp::ast::Atom::Literal(::conjure_cp::ast::Literal::Int(#constant_value)),
                    )})
                }
                "TRUE" => Ok(quote! {::conjure_cp::ast::Expression::Atomic(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Atom::Literal(::conjure_cp::ast::Literal::Bool(true)),
                )}),
                "FALSE" => Ok(quote! {::conjure_cp::ast::Expression::Atomic(
                    ::conjure_cp::ast::Metadata::new(),
                    ::conjure_cp::ast::Atom::Literal(::conjure_cp::ast::Literal::Bool(false)),
                )}),
                _ => Err(EssenceParseError::syntax_error(
                    format!("Unsupported constant kind: {}", child.kind()),
                    Some(child.range()),
                )),
            }
        }
        "variable" => {
            let variable_name =
                String::from(&source_code[constraint.start_byte()..constraint.end_byte()]);
            Ok(quote! {::conjure_cp::ast::Expression::Atomic(
                ::conjure_cp::ast::Metadata::new(),
                ::conjure_cp::ast::Atom::new_ref(#variable_name),
            )})
        }
        "from_solution" => match root.kind() {
            "dominance_relation" => {
                let inner_ts = child_expr_to_ts(constraint, source_code, root)?;
                Ok(quote! {
                    ::conjure_cp::ast::Expression::FromSolution(::conjure_cp::ast::Metadata::new(), ::conjure_cp::ast::Moo::new(#inner_ts))
                })
            }
            _ => Err(EssenceParseError::syntax_error(
                "`fromSolution()` is only allowed inside dominance relation definitions"
                    .to_string(),
                Some(constraint.range()),
            )),
        },
        "metavar" => {
            let inner = constraint
                .named_child(0)
                .ok_or(EssenceParseError::syntax_error(
                    "Expected name for meta-variable".to_string(),
                    Some(constraint.range()),
                ))?;
            let name = &source_code[inner.start_byte()..inner.end_byte()];
            let ident = Ident::new(name, Span::call_site());
            Ok(quote! {#ident.clone().into()})
        }
        "ERROR" => {
            let expr = &source_code[constraint.start_byte()..constraint.end_byte()];
            Err(EssenceParseError::syntax_error(
                format!("`{expr}` is not a valid expression"),
                Some(constraint.range()),
            ))
        }
        _ => Err(EssenceParseError::syntax_error(
            format!("{} is not a recognized node kind", constraint.kind()),
            Some(constraint.range()),
        )),
    }
}

fn child_expr_to_ts(
    node: Node,
    source_code: &str,
    root: &Node,
) -> Result<TokenStream, EssenceParseError> {
    match node.named_child(0) {
        Some(child) => parse_expr_to_ts(child, source_code, root),
        None => Err(EssenceParseError::syntax_error(
            format!("Missing node in expression of kind {}", node.kind()),
            Some(node.range()),
        )),
    }
}

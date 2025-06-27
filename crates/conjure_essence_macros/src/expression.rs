use conjure_essence_parser::errors::EssenceParseError;
use conjure_essence_parser::expression::child_expr;
use conjure_essence_parser::util::named_children;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;
use tree_sitter::Node;

// TODO (gskorokhod) - This is a lot of code duplication, but I don't see an easy way to avoid it short of a ~visitor pattern~ of some sort.

/// "Sister function" to conjure_essence_parser::::conjure_core::ast::Expression::parse_::conjure_core::ast::Expression.
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
            Ok(quote! {::conjure_core::ast::Expression::Not(
                ::conjure_core::metadata::Metadata::new(),
                Box::new(#child),
            )})
        }
        "abs_value" => {
            let child = child_expr_to_ts(constraint, source_code, root)?;
            Ok(quote! {
                ::conjure_core::ast::Expression::Abs(
                ::conjure_core::metadata::Metadata::new(),
                Box::new(#child),
            )})
        }
        "negative_expr" => {
            let child = child_expr_to_ts(constraint, source_code, root)?;
            Ok(quote! {::conjure_core::ast::Expression::Neg(
                ::conjure_core::metadata::Metadata::new(),
                Box::new(#child),
            )})
        }
        "exponent" | "product_expr" | "sum_expr" | "comparison" | "and_expr" | "or_expr"
        | "implication" => {
            let expr1 = child_expr_to_ts(constraint, source_code, root)?;
            let op = constraint.child(1).ok_or(format!(
                "Missing operator in expression {}",
                constraint.kind()
            ))?;
            let op_type = &source_code[op.start_byte()..op.end_byte()];
            let expr2_node = constraint.child(2).ok_or(format!(
                "Missing second operand in expression {}",
                constraint.kind()
            ))?;
            let expr2 = parse_expr_to_ts(expr2_node, source_code, root)?;

            match op_type {
                "**" => Ok(quote! {::conjure_core::ast::Expression::UnsafePow(
                    ::conjure_core::metadata::Metadata::new(),
                    Box::new(#expr1),
                    Box::new(#expr2),
                )}),
                "+" => Ok(quote! {::conjure_core::ast::Expression::Sum(
                    ::conjure_core::metadata::Metadata::new(),
                    Box::new(::conjure_core::matrix_expr![#expr1, #expr2]),
                )}),
                "-" => Ok(quote! {::conjure_core::ast::Expression::Minus(
                    ::conjure_core::metadata::Metadata::new(),
                    Box::new(#expr1),
                    Box::new(#expr2),
                )}),
                "*" => Ok(
                    quote! {::conjure_core::ast::Expression::Product(::conjure_core::metadata::Metadata::new(), Box::new(::conjure_core::matrix_expr![#expr1, #expr2]))},
                ),
                "/" => {
                    //TODO: add checks for if division is safe or not
                    Ok(quote! {::conjure_core::ast::Expression::UnsafeDiv(
                        ::conjure_core::metadata::Metadata::new(),
                        Box::new(#expr1),
                        Box::new(#expr2),
                    )})
                }
                "%" => {
                    //TODO: add checks for if mod is safe or not
                    Ok(quote! {::conjure_core::ast::Expression::UnsafeMod(
                        ::conjure_core::metadata::Metadata::new(),
                        Box::new(#expr1),
                        Box::new(#expr2),
                    )})
                }
                "=" => Ok(quote! {::conjure_core::ast::Expression::Eq(
                    ::conjure_core::metadata::Metadata::new(),
                    Box::new(#expr1),
                    Box::new(#expr2),
                )}),
                "!=" => Ok(quote! {::conjure_core::ast::Expression::Neq(
                    ::conjure_core::metadata::Metadata::new(),
                    Box::new(#expr1),
                    Box::new(#expr2),
                )}),
                "<=" => Ok(quote! {::conjure_core::ast::Expression::Leq(
                    ::conjure_core::metadata::Metadata::new(),
                    Box::new(#expr1),
                    Box::new(#expr2),
                )}),
                ">=" => Ok(quote! {::conjure_core::ast::Expression::Geq(
                    ::conjure_core::metadata::Metadata::new(),
                    Box::new(#expr1),
                    Box::new(#expr2),
                )}),
                "<" => Ok(quote! {::conjure_core::ast::Expression::Lt(
                    ::conjure_core::metadata::Metadata::new(),
                    Box::new(#expr1),
                    Box::new(#expr2),
                )}),
                ">" => Ok(quote! {::conjure_core::ast::Expression::Gt(
                    ::conjure_core::metadata::Metadata::new(),
                    Box::new(#expr1),
                    Box::new(#expr2),
                )}),
                "/\\" => Ok(quote! {::conjure_core::ast::Expression::And(
                    ::conjure_core::metadata::Metadata::new(),
                    Box::new(::conjure_core::matrix_expr![#expr1, #expr2]),
                )}),
                "\\/" => Ok(quote! {::conjure_core::ast::Expression::Or(
                    ::conjure_core::metadata::Metadata::new(),
                    Box::new(::conjure_core::matrix_expr![#expr1, #expr2]),
                )}),
                "->" => Ok(quote! {::conjure_core::ast::Expression::Imply(
                    ::conjure_core::metadata::Metadata::new(),
                    Box::new(#expr1),
                    Box::new(#expr2),
                )}),
                _ => Err(format!("Unsupported operator '{op_type}'").into()),
            }
        }
        "quantifier_expr" => {
            let mut expr_list = Vec::new();
            for expr in named_children(&constraint) {
                expr_list.push(parse_expr_to_ts(expr, source_code, root)?);
            }

            let quantifier = constraint.child(0).ok_or(format!(
                "Missing quantifier in expression {}",
                constraint.kind()
            ))?;
            let quantifier_type = &source_code[quantifier.start_byte()..quantifier.end_byte()];

            match quantifier_type {
                "and" => Ok(quote! {::conjure_core::ast::Expression::And(
                    ::conjure_core::metadata::Metadata::new(),
                    Box::new(::conjure_core::matrix_expr![#(#expr_list),*]),
                )}),
                "or" => Ok(quote! {::conjure_core::ast::Expression::Or(
                    ::conjure_core::metadata::Metadata::new(),
                    Box::new(::conjure_core::matrix_expr![#(#expr_list),*]),
                )}),
                "min" => Ok(quote! {::conjure_core::ast::Expression::Min(
                    ::conjure_core::metadata::Metadata::new(),
                    Box::new(::conjure_core::matrix_expr![#(#expr_list),*]),
                )}),
                "max" => Ok(quote! {::conjure_core::ast::Expression::Max(
                    ::conjure_core::metadata::Metadata::new(),
                    Box::new(::conjure_core::matrix_expr![#(#expr_list),*]),
                )}),
                "sum" => Ok(quote! {::conjure_core::ast::Expression::Sum(
                    ::conjure_core::metadata::Metadata::new(),
                    Box::new(::conjure_core::matrix_expr![#(#expr_list),*]),
                )}),
                "allDiff" => Ok(quote! {::conjure_core::ast::Expression::AllDiff(
                    ::conjure_core::metadata::Metadata::new(),
                    Box::new(::conjure_core::matrix_expr![#(#expr_list),*]),
                )}),
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
                    Ok(quote! {::conjure_core::ast::Expression::Atomic(
                        ::conjure_core::metadata::Metadata::new(),
                        ::conjure_core::ast::Atom::Literal(::conjure_core::ast::Literal::Int(#constant_value)),
                    )})
                }
                "TRUE" => Ok(quote! {::conjure_core::ast::Expression::Atomic(
                    ::conjure_core::metadata::Metadata::new(),
                    ::conjure_core::ast::Atom::Literal(::conjure_core::ast::Literal::Bool(true)),
                )}),
                "FALSE" => Ok(quote! {::conjure_core::ast::Expression::Atomic(
                    ::conjure_core::metadata::Metadata::new(),
                    ::conjure_core::ast::Atom::Literal(::conjure_core::ast::Literal::Bool(false)),
                )}),
                _ => Err(format!("Unsupported constant kind: {}", child.kind()).into()),
            }
        }
        "variable" => {
            let variable_name =
                String::from(&source_code[constraint.start_byte()..constraint.end_byte()]);
            Ok(quote! {::conjure_core::ast::Expression::Atomic(
                ::conjure_core::metadata::Metadata::new(),
                ::conjure_core::ast::Atom::Reference(::conjure_core::ast::Name::User(#variable_name.into())),
            )})
        }
        "from_solution" => match root.kind() {
            "dominance_relation" => {
                let inner_ts = child_expr_to_ts(constraint, source_code, root)?;
                let inner = child_expr(constraint, source_code, root)?;
                match inner {
                    ::conjure_core::ast::Expression::Atomic(_, _) => Ok(quote! {
                        ::conjure_core::ast::Expression::FromSolution(::conjure_core::metadata::Metadata::new(), Box::new(#inner_ts))
                    }),
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
        "metavar" => {
            let inner = constraint
                .named_child(0)
                .ok_or("Expected name for meta-variable".to_string())?;
            let name = &source_code[inner.start_byte()..inner.end_byte()];
            let ident = Ident::new(name, Span::call_site());
            Ok(quote! {#ident.clone().into()})
        }
        _ => Err(format!("{} is not a recognized node kind", constraint.kind()).into()),
    }
}

fn child_expr_to_ts(
    node: Node,
    source_code: &str,
    root: &Node,
) -> Result<TokenStream, EssenceParseError> {
    match node.named_child(0) {
        Some(child) => parse_expr_to_ts(child, source_code, root),
        None => Err(format!("Missing node in expression of kind {}", node.kind()).into()),
    }
}

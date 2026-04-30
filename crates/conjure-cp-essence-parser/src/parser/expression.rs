use crate::diagnostics::diagnostics_api::SymbolKind;
use crate::errors::{FatalParseError, RecoverableParseError};
use crate::parser::ParseContext;
use crate::parser::atom::parse_atom;
use crate::parser::comprehension::parse_quantifier_or_aggregate_expr;
use crate::util::TypecheckingContext;
use crate::{child, field, named_child};
use conjure_cp_core::ast::{Expression, GroundDomain, Metadata, Moo};
use conjure_cp_core::{domain_int, matrix_expr, range};
use tree_sitter::Node;

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
                        &ctx.source_code[node.start_byte()..node.end_byte()]
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
                        &ctx.source_code[node.start_byte()..node.end_byte()]
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
                        &ctx.source_code[node.start_byte()..node.end_byte()]
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
                    format!("Type error: {}\n\tExepected: arithmetic expression\n\tFound: comparison expression", &ctx.source_code[node.start_byte()..node.end_byte()]),
                    Some(node.range()),
                ));
                return Ok(None);
            }
            ctx.typechecking_context = TypecheckingContext::Matrix;
            parse_all_diff_comparison(ctx, &node)
        }
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Unexpected expression type: '{}'", node.kind()),
                Some(node.range()),
            ));
            Ok(None)
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
            ctx.typechecking_context = TypecheckingContext::Matrix;
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

    let expr = match operator_str {
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
    };

    if expr.is_ok() {
        ctx.add_span_and_doc_hover(
            &operator_node,
            operator_str,
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
    let Some(op_node) = field!(recover, ctx, node, "operator") else {
        return Ok(None);
    };
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
            // No documentation for or in Bits yet
            doc_name = "or";
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

        "=" => {
            doc_name = "L_Eq"; //no docs yet
            Ok(Some(Expression::Eq(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "!=" => {
            doc_name = "L_Neq"; //no docs yet
            Ok(Some(Expression::Neq(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "<=" => {
            doc_name = "L_Leq"; //no docs yet
            Ok(Some(Expression::Leq(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        ">=" => {
            doc_name = "L_Geq"; //no docs yet
            Ok(Some(Expression::Geq(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "<" => {
            doc_name = "L_Lt"; //no docs yet
            Ok(Some(Expression::Lt(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        ">" => {
            doc_name = "L_Gt"; //no docs yet
            Ok(Some(Expression::Gt(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }

        "->" => {
            doc_name = "L_Imply"; //no docs yet
            Ok(Some(Expression::Imply(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "<->" => {
            doc_name = "L_Iff"; //no docs yet
            Ok(Some(Expression::Iff(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "<lex" => {
            doc_name = "L_LexLt"; //no docs yet
            Ok(Some(Expression::LexLt(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        ">lex" => {
            doc_name = "L_LexGt"; //no docs yet
            Ok(Some(Expression::LexGt(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        "<=lex" => {
            doc_name = "L_LexLeq"; //no docs yet
            Ok(Some(Expression::LexLeq(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            )))
        }
        ">=lex" => {
            doc_name = "L_LexGeq"; //no docs yet
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
        ctx.add_span_and_doc_hover(&op_node, doc_name, SymbolKind::Function, None, None);
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
        GroundDomain::Sequence(_, _) => TypecheckingContext::Sequence,
        GroundDomain::Function(_, _, _)
        | GroundDomain::Variant(_)
        | GroundDomain::Relation(_, _)
        | GroundDomain::Empty(_) => TypecheckingContext::Unknown,
    }
}

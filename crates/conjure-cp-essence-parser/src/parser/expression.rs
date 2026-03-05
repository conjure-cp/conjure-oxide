use crate::diagnostics::diagnostics_api::SymbolKind;
use crate::diagnostics::source_map::{HoverInfo, span_with_hover};
use crate::errors::FatalParseError;
use crate::parser::ParseContext;
use crate::parser::atom::parse_atom;
use crate::parser::comprehension::parse_quantifier_or_aggregate_expr;
use crate::{field, named_child};
use conjure_cp_core::ast::{Expression, Metadata, Moo};
use conjure_cp_core::{domain_int, matrix_expr, range};
use tree_sitter::Node;

/// Parse an Essence expression into its Conjure AST representation.
pub fn parse_expression(
    ctx: &mut ParseContext,
    node: Node,
) -> Result<Option<Expression>, FatalParseError> {
    match node.kind() {
        "atom" => parse_atom(ctx, &node),
        "bool_expr" => parse_boolean_expression(ctx, &node),
        "arithmetic_expr" => parse_arithmetic_expression(ctx, &node),
        "comparison_expr" => parse_binary_expression(ctx, &node),
        "dominance_relation" => parse_dominance_relation(ctx, &node),
        _ => Err(FatalParseError::internal_error(
            format!("Unexpected expression type: '{}'", node.kind()),
            Some(node.range()),
        )),
    }
}

fn parse_dominance_relation(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    if ctx.root.kind() == "dominance_relation" {
        return Err(FatalParseError::internal_error(
            "Nested dominance relations are not allowed".to_string(),
            Some(node.range()),
        ));
    }

    // NB: In all other cases, we keep the root the same;
    // However, here we create a new context with the new root so downstream functions
    // know we are inside a dominance relation
    let mut inner_ctx = ParseContext {
        source_code: ctx.source_code,
        root: node,
        symbols: ctx.symbols.clone(),
        errors: ctx.errors,
        source_map: &mut *ctx.source_map,
    };

    let Some(inner) = parse_expression(&mut inner_ctx, field!(node, "expression"))? else {
        return Ok(None);
    };

    Ok(Some(Expression::DominanceRelation(
        Metadata::new(),
        Moo::new(inner),
    )))
}

fn parse_arithmetic_expression(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    let inner = named_child!(node);
    match inner.kind() {
        "atom" => parse_atom(ctx, &inner),
        "negative_expr" | "abs_value" | "sub_arith_expr" | "toInt_expr" => {
            parse_unary_expression(ctx, &inner)
        }
        "exponent" | "product_expr" | "sum_expr" => parse_binary_expression(ctx, &inner),
        "list_combining_expr_arith" => parse_list_combining_expression(ctx, &inner),
        "aggregate_expr" => parse_quantifier_or_aggregate_expr(ctx, &inner),
        _ => Err(FatalParseError::internal_error(
            format!("Expected arithmetic expression, found: {}", inner.kind()),
            Some(inner.range()),
        )),
    }
}

fn parse_boolean_expression(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    let inner = named_child!(node);
    match inner.kind() {
        "atom" => parse_atom(ctx, &inner),
        "not_expr" | "sub_bool_expr" => parse_unary_expression(ctx, &inner),
        "and_expr" | "or_expr" | "implication" | "iff_expr" | "set_operation_bool" => {
            parse_binary_expression(ctx, &inner)
        }
        "list_combining_expr_bool" => parse_list_combining_expression(ctx, &inner),
        "quantifier_expr" => parse_quantifier_or_aggregate_expr(ctx, &inner),
        _ => Err(FatalParseError::internal_error(
            format!("Expected boolean expression, found '{}'", inner.kind()),
            Some(inner.range()),
        )),
    }
}

fn parse_list_combining_expression(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    let operator_node = field!(node, "operator");
    let operator_str = &ctx.source_code[operator_node.start_byte()..operator_node.end_byte()];

    let Some(inner) = parse_atom(ctx, &field!(node, "arg"))? else {
        return Ok(None);
    };

    match operator_str {
        "and" => Ok(Some(Expression::And(Metadata::new(), Moo::new(inner)))),
        "or" => Ok(Some(Expression::Or(Metadata::new(), Moo::new(inner)))),
        "sum" => Ok(Some(Expression::Sum(Metadata::new(), Moo::new(inner)))),
        "product" => Ok(Some(Expression::Product(Metadata::new(), Moo::new(inner)))),
        "min" => Ok(Some(Expression::Min(Metadata::new(), Moo::new(inner)))),
        "max" => Ok(Some(Expression::Max(Metadata::new(), Moo::new(inner)))),
        "allDiff" => Ok(Some(Expression::AllDiff(Metadata::new(), Moo::new(inner)))),
        _ => Err(FatalParseError::internal_error(
            format!("Invalid operator: '{operator_str}'"),
            Some(operator_node.range()),
        )),
    }
}

fn parse_unary_expression(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    let Some(inner) = parse_expression(ctx, field!(node, "expression"))? else {
        return Ok(None);
    };
    match node.kind() {
        "negative_expr" => Ok(Some(Expression::Neg(Metadata::new(), Moo::new(inner)))),
        "abs_value" => Ok(Some(Expression::Abs(Metadata::new(), Moo::new(inner)))),
        "not_expr" => Ok(Some(Expression::Not(Metadata::new(), Moo::new(inner)))),
        "toInt_expr" => Ok(Some(Expression::ToInt(Metadata::new(), Moo::new(inner)))),
        "sub_bool_expr" | "sub_arith_expr" => Ok(Some(inner)),
        _ => Err(FatalParseError::internal_error(
            format!("Unrecognised unary operation: '{}'", node.kind()),
            Some(node.range()),
        )),
    }
}

pub fn parse_binary_expression(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    let mut parse_subexpr = |expr: Node| parse_expression(ctx, expr);

    let Some(left) = parse_subexpr(field!(node, "left"))? else {
        return Ok(None);
    };
    let Some(right) = parse_subexpr(field!(node, "right"))? else {
        return Ok(None);
    };

    let op_node = field!(node, "operator");
    let op_str = &ctx.source_code[op_node.start_byte()..op_node.end_byte()];

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
        _ => Err(FatalParseError::internal_error(
            format!("Invalid operator: '{op_str}'"),
            Some(op_node.range()),
        )),
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

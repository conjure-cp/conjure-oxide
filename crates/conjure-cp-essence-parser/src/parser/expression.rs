use crate::errors::EssenceParseError;
use crate::parser::atom::parse_atom;
use crate::{field, named_child};
use conjure_cp_core::ast::{Expression, Metadata, Moo, SymbolTable};
use conjure_cp_core::{domain_int, matrix_expr, range};
use std::cell::RefCell;
use std::rc::Rc;
use tree_sitter::Node;

/// Parse an Essence expression into its Conjure AST representation.
pub fn parse_expression(
    node: Node,
    source_code: &str,
    root: &Node,
    symbols_ptr: Option<&Rc<RefCell<SymbolTable>>>,
) -> Result<Expression, EssenceParseError> {
    match node.kind() {
        "atom" => parse_atom(&node, source_code, root, symbols_ptr),
        "bool_expr" => parse_boolean_expression(&node, source_code, root, symbols_ptr),
        "arithmetic_expr" => parse_arithmetic_expression(&node, source_code, root, symbols_ptr),
        "comparison_expr" => parse_binary_expression(&node, source_code, root, symbols_ptr),
        "dominance_relation" => parse_dominance_relation(&node, source_code, root, symbols_ptr),
        "ERROR" => Err(EssenceParseError::syntax_error(
            format!(
                "'{}' is not a valid expression",
                &source_code[node.start_byte()..node.end_byte()]
            ),
            Some(node.range()),
        )),
        _ => Err(EssenceParseError::syntax_error(
            format!("Unknown expression kind: '{}'", node.kind()),
            Some(node.range()),
        )),
    }
}

fn parse_dominance_relation(
    node: &Node,
    source_code: &str,
    root: &Node,
    symbols_ptr: Option<&Rc<RefCell<SymbolTable>>>,
) -> Result<Expression, EssenceParseError> {
    if root.kind() == "dominance_relation" {
        return Err(EssenceParseError::syntax_error(
            "Nested dominance relations are not allowed".to_string(),
            Some(node.range()),
        ));
    }

    // NB: In all other cases, we keep the root the same;
    // However, here we set the new root to `node` so downstream functions
    // know we are inside a dominance relation
    let inner = parse_expression(field!(node, "expression"), source_code, node, symbols_ptr)?;
    Ok(Expression::DominanceRelation(
        Metadata::new(),
        Moo::new(inner),
    ))
}

fn parse_arithmetic_expression(
    node: &Node,
    source_code: &str,
    root: &Node,
    symbols_ptr: Option<&Rc<RefCell<SymbolTable>>>,
) -> Result<Expression, EssenceParseError> {
    let inner = named_child!(node);
    match inner.kind() {
        "atom" => parse_atom(&inner, source_code, root, symbols_ptr),
        "negative_expr" | "abs_value" | "sub_arith_expr" | "toInt_expr" => {
            parse_unary_expression(&inner, source_code, root, symbols_ptr)
        }
        "exponent" | "product_expr" | "sum_expr" => {
            parse_binary_expression(&inner, source_code, root, symbols_ptr)
        }
        "quantifier_expr_arith" => {
            parse_quantifier_expression(&inner, source_code, root, symbols_ptr)
        }
        _ => Err(EssenceParseError::syntax_error(
            format!("Expected arithmetic expression, found: {}", inner.kind()),
            Some(inner.range()),
        )),
    }
}

fn parse_boolean_expression(
    node: &Node,
    source_code: &str,
    root: &Node,
    symbols_ptr: Option<&Rc<RefCell<SymbolTable>>>,
) -> Result<Expression, EssenceParseError> {
    let inner = named_child!(node);
    match inner.kind() {
        "atom" => parse_atom(&inner, source_code, root, symbols_ptr),
        "not_expr" | "sub_bool_expr" => {
            parse_unary_expression(&inner, source_code, root, symbols_ptr)
        }
        "and_expr" | "or_expr" | "implication" | "iff_expr" | "set_operation_bool" => {
            parse_binary_expression(&inner, source_code, root, symbols_ptr)
        }
        "quantifier_expr_bool" => {
            parse_quantifier_expression(&inner, source_code, root, symbols_ptr)
        }
        _ => Err(EssenceParseError::syntax_error(
            format!("Expected boolean expression, found '{}'", inner.kind()),
            Some(inner.range()),
        )),
    }
}

fn parse_quantifier_expression(
    node: &Node,
    source_code: &str,
    root: &Node,
    symbols_ptr: Option<&Rc<RefCell<SymbolTable>>>,
) -> Result<Expression, EssenceParseError> {
    // TODO (terminology) - this is not really a quantifier, just a list operation.
    // Quantifiers are things like:
    // forAll <name> : <domain> . <expr>

    let quantifier_node = field!(node, "quantifier");
    let quantifier_str = &source_code[quantifier_node.start_byte()..quantifier_node.end_byte()];

    let inner = parse_atom(&field!(node, "arg"), source_code, root, symbols_ptr)?;

    match quantifier_str {
        "and" => Ok(Expression::And(Metadata::new(), Moo::new(inner))),
        "or" => Ok(Expression::Or(Metadata::new(), Moo::new(inner))),
        "sum" => Ok(Expression::Sum(Metadata::new(), Moo::new(inner))),
        "product" => Ok(Expression::Product(Metadata::new(), Moo::new(inner))),
        "min" => Ok(Expression::Min(Metadata::new(), Moo::new(inner))),
        "max" => Ok(Expression::Max(Metadata::new(), Moo::new(inner))),
        "allDiff" => Ok(Expression::AllDiff(Metadata::new(), Moo::new(inner))),
        _ => Err(EssenceParseError::syntax_error(
            format!("Invalid quantifier: '{quantifier_str}'"),
            Some(quantifier_node.range()),
        )),
    }
}

fn parse_unary_expression(
    node: &Node,
    source_code: &str,
    root: &Node,
    symbols_ptr: Option<&Rc<RefCell<SymbolTable>>>,
) -> Result<Expression, EssenceParseError> {
    let inner = parse_expression(field!(node, "expression"), source_code, root, symbols_ptr)?;
    match node.kind() {
        "negative_expr" => Ok(Expression::Neg(Metadata::new(), Moo::new(inner))),
        "abs_value" => Ok(Expression::Abs(Metadata::new(), Moo::new(inner))),
        "not_expr" => Ok(Expression::Not(Metadata::new(), Moo::new(inner))),
        "toInt_expr" => Ok(Expression::ToInt(Metadata::new(), Moo::new(inner))),
        "sub_bool_expr" | "sub_arith_expr" => Ok(inner),
        _ => Err(EssenceParseError::syntax_error(
            format!("Unrecognised unary operation: '{}'", node.kind()),
            Some(node.range()),
        )),
    }
}

pub fn parse_binary_expression(
    node: &Node,
    source_code: &str,
    root: &Node,
    symbols_ptr: Option<&Rc<RefCell<SymbolTable>>>,
) -> Result<Expression, EssenceParseError> {
    let parse_subexpr = |expr: Node| parse_expression(expr, source_code, root, symbols_ptr);

    let left = parse_subexpr(field!(node, "left"))?;
    let right = parse_subexpr(field!(node, "right"))?;

    let op_node = field!(node, "operator");
    let op_str = &source_code[op_node.start_byte()..op_node.end_byte()];

    match op_str {
        // NB: We are deliberately setting the index domain to 1.., not 1..2.
        // Semantically, this means "a list that can grow/shrink arbitrarily".
        // This is expected by rules which will modify the terms of the sum expression
        // (e.g. by partially evaluating them).
        "+" => Ok(Expression::Sum(
            Metadata::new(),
            Moo::new(matrix_expr![left, right; domain_int!(1..)]),
        )),
        "-" => Ok(Expression::Minus(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        )),
        "*" => Ok(Expression::Product(
            Metadata::new(),
            Moo::new(matrix_expr![left, right; domain_int!(1..)]),
        )),
        "/\\" => Ok(Expression::And(
            Metadata::new(),
            Moo::new(matrix_expr![left, right; domain_int!(1..)]),
        )),
        "\\/" => Ok(Expression::Or(
            Metadata::new(),
            Moo::new(matrix_expr![left, right; domain_int!(1..)]),
        )),
        "**" => Ok(Expression::UnsafePow(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        )),
        "/" => {
            //TODO: add checks for if division is safe or not
            Ok(Expression::UnsafeDiv(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ))
        }
        "%" => {
            //TODO: add checks for if mod is safe or not
            Ok(Expression::UnsafeMod(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ))
        }
        "=" => Ok(Expression::Eq(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        )),
        "!=" => Ok(Expression::Neq(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        )),
        "<=" => Ok(Expression::Leq(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        )),
        ">=" => Ok(Expression::Geq(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        )),
        "<" => Ok(Expression::Lt(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        )),
        ">" => Ok(Expression::Gt(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        )),
        "->" => Ok(Expression::Imply(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        )),
        "<->" => Ok(Expression::Iff(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        )),
        "in" => Ok(Expression::In(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        )),
        "subset" => Ok(Expression::Subset(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        )),
        "subsetEq" => Ok(Expression::SubsetEq(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        )),
        "supset" => Ok(Expression::Supset(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        )),
        "supsetEq" => Ok(Expression::SupsetEq(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        )),
        "union" => Ok(Expression::Union(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        )),
        "intersect" => Ok(Expression::Intersect(
            Metadata::new(),
            Moo::new(left),
            Moo::new(right),
        )),
        _ => Err(EssenceParseError::syntax_error(
            format!("Invalid operator: '{op_str}'"),
            Some(op_node.range()),
        )),
    }
}

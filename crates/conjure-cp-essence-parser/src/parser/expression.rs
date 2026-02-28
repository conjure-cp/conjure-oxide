use crate::diagnostics::diagnostics_api::SymbolKind;
use crate::diagnostics::source_map::{HoverInfo, SourceMap, span_with_hover};
use crate::errors::EssenceParseError;
use crate::parser::atom::parse_atom;
use crate::parser::comprehension::parse_quantifier_or_aggregate_expr;
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
    symbols_ptr: Option<Rc<RefCell<SymbolTable>>>,
    source_map: &mut SourceMap,
) -> Result<Expression, EssenceParseError> {
    match node.kind() {
        "atom" => parse_atom(&node, source_code, root, symbols_ptr, source_map),
        "bool_expr" => parse_boolean_expression(&node, source_code, root, symbols_ptr, source_map),
        "arithmetic_expr" => {
            parse_arithmetic_expression(&node, source_code, root, symbols_ptr, source_map)
        }
        "comparison_expr" => {
            parse_binary_expression(&node, source_code, root, symbols_ptr, source_map)
        }
        "dominance_relation" => {
            parse_dominance_relation(&node, source_code, root, symbols_ptr, source_map)
        }
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
    symbols_ptr: Option<Rc<RefCell<SymbolTable>>>,
    source_map: &mut SourceMap,
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
    let inner = parse_expression(
        field!(node, "expression"),
        source_code,
        node,
        symbols_ptr,
        source_map,
    )?;
    Ok(Expression::DominanceRelation(
        Metadata::new(),
        Moo::new(inner),
    ))
}

fn parse_arithmetic_expression(
    node: &Node,
    source_code: &str,
    root: &Node,
    symbols_ptr: Option<Rc<RefCell<SymbolTable>>>,
    source_map: &mut SourceMap,
) -> Result<Expression, EssenceParseError> {
    let inner = named_child!(node);
    match inner.kind() {
        "atom" => parse_atom(&inner, source_code, root, symbols_ptr, source_map),
        "negative_expr" | "abs_value" | "sub_arith_expr" | "toInt_expr" => {
            parse_unary_expression(&inner, source_code, root, symbols_ptr, source_map)
        }
        "exponent" | "product_expr" | "sum_expr" => {
            parse_binary_expression(&inner, source_code, root, symbols_ptr, source_map)
        }
        "list_combining_expr_arith" => {
            parse_list_combining_expression(&inner, source_code, root, symbols_ptr, source_map)
        }
        "aggregate_expr" => {
            parse_quantifier_or_aggregate_expr(&inner, source_code, root, symbols_ptr, source_map)
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
    symbols_ptr: Option<Rc<RefCell<SymbolTable>>>,
    source_map: &mut SourceMap,
) -> Result<Expression, EssenceParseError> {
    let inner = named_child!(node);
    match inner.kind() {
        "atom" => parse_atom(&inner, source_code, root, symbols_ptr, source_map),
        "not_expr" | "sub_bool_expr" => {
            parse_unary_expression(&inner, source_code, root, symbols_ptr, source_map)
        }
        "and_expr" | "or_expr" | "implication" | "iff_expr" | "set_operation_bool" => {
            parse_binary_expression(&inner, source_code, root, symbols_ptr, source_map)
        }
        "list_combining_expr_bool" => {
            parse_list_combining_expression(&inner, source_code, root, symbols_ptr, source_map)
        }
        "quantifier_expr" => {
            parse_quantifier_or_aggregate_expr(&inner, source_code, root, symbols_ptr, source_map)
        }
        _ => Err(EssenceParseError::syntax_error(
            format!("Expected boolean expression, found '{}'", inner.kind()),
            Some(inner.range()),
        )),
    }
}

fn parse_list_combining_expression(
    node: &Node,
    source_code: &str,
    root: &Node,
    symbols_ptr: Option<Rc<RefCell<SymbolTable>>>,
    source_map: &mut SourceMap,
) -> Result<Expression, EssenceParseError> {
    let operator_node = field!(node, "operator");
    let operator_str = &source_code[operator_node.start_byte()..operator_node.end_byte()];

    let inner = parse_atom(
        &field!(node, "arg"),
        source_code,
        root,
        symbols_ptr,
        source_map,
    )?;

    match operator_str {
        "and" => Ok(Expression::And(Metadata::new(), Moo::new(inner))),
        "or" => Ok(Expression::Or(Metadata::new(), Moo::new(inner))),
        "sum" => Ok(Expression::Sum(Metadata::new(), Moo::new(inner))),
        "product" => Ok(Expression::Product(Metadata::new(), Moo::new(inner))),
        "min" => Ok(Expression::Min(Metadata::new(), Moo::new(inner))),
        "max" => Ok(Expression::Max(Metadata::new(), Moo::new(inner))),
        "allDiff" => Ok(Expression::AllDiff(Metadata::new(), Moo::new(inner))),
        _ => Err(EssenceParseError::syntax_error(
            format!("Invalid operator: '{operator_str}'"),
            Some(operator_node.range()),
        )),
    }
}

fn parse_unary_expression(
    node: &Node,
    source_code: &str,
    root: &Node,
    symbols_ptr: Option<Rc<RefCell<SymbolTable>>>,
    source_map: &mut SourceMap,
) -> Result<Expression, EssenceParseError> {
    let inner = parse_expression(
        field!(node, "expression"),
        source_code,
        root,
        symbols_ptr,
        source_map,
    )?;
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
    symbols_ptr: Option<Rc<RefCell<SymbolTable>>>,
    source_map: &mut SourceMap,
) -> Result<Expression, EssenceParseError> {
    let mut parse_subexpr =
        |expr: Node| parse_expression(expr, source_code, root, symbols_ptr.clone(), source_map);

    let left = parse_subexpr(field!(node, "left"))?;
    let right = parse_subexpr(field!(node, "right"))?;

    let op_node = field!(node, "operator");
    let op_str = &source_code[op_node.start_byte()..op_node.end_byte()];

    let mut description = "";
    let expr;

    match op_str {
        // NB: We are deliberately setting the index domain to 1.., not 1..2.
        // Semantically, this means "a list that can grow/shrink arbitrarily".
        // This is expected by rules which will modify the terms of the sum expression
        // (e.g. by partially evaluating them).
        "+" => {
            description = "sum: aggregate operator that sums over a list of terms";
            expr = Ok(Expression::Sum(
                Metadata::new(),
                Moo::new(matrix_expr![left, right; domain_int!(1..)]),
            ));
        }
        "-" => {
            description = "difference: operation that subtracts one term from another";
            expr = Ok(Expression::Minus(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        "*" => {
            description = "product: aggregate operator that multiplies a list of terms";
            expr = Ok(Expression::Product(
                Metadata::new(),
                Moo::new(matrix_expr![left, right; domain_int!(1..)]),
            ));
        }
        "/\\" => {
            description = "logical conjunction: true when both operands hold";
            expr = Ok(Expression::And(
                Metadata::new(),
                Moo::new(matrix_expr![left, right; domain_int!(1..)]),
            ));
        }
        "\\/" => {
            description = "logical disjunction: true when at least one operand holds";
            expr = Ok(Expression::Or(
                Metadata::new(),
                Moo::new(matrix_expr![left, right; domain_int!(1..)]),
            ));
        }
        "**" => {
            description = "exponentiation: raises the left operand to the right power";
            expr = Ok(Expression::UnsafePow(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        "/" => {
            //TODO: add checks for if division is safe or not
            description = "division: divides the left operand by the right operand";
            expr = Ok(Expression::UnsafeDiv(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        "%" => {
            //TODO: add checks for if mod is safe or not
            description = "modulus: remainder when dividing the left operand by the right operand";
            expr = Ok(Expression::UnsafeMod(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        "=" => {
            description = "equality comparison: succeeds when both operands are identical";
            expr = Ok(Expression::Eq(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        "!=" => {
            description = "inequality comparison: succeeds when operands differ";
            expr = Ok(Expression::Neq(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        "<=" => {
            description = "less-than-or-equal comparison";
            expr = Ok(Expression::Leq(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        ">=" => {
            description = "greater-than-or-equal comparison";
            expr = Ok(Expression::Geq(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        "<" => {
            description = "strict less-than comparison";
            expr = Ok(Expression::Lt(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        ">" => {
            description = "strict greater-than comparison";
            expr = Ok(Expression::Gt(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        "->" => {
            description =
                "implication: false only when the antecedent is true and the consequent is false";
            expr = Ok(Expression::Imply(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        "<->" => {
            description = "logical equivalence: true when both operands share the same truth value";
            expr = Ok(Expression::Iff(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        "<lex" => {
            description = "lexicographic less-than comparison";
            expr = Ok(Expression::LexLt(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        ">lex" => {
            description = "lexicographic greater-than comparison";
            expr = Ok(Expression::LexGt(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        "<=lex" => {
            description = "lexicographic less-than-or-equal comparison";
            expr = Ok(Expression::LexLeq(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        ">=lex" => {
            description = "lexicographic greater-than-or-equal comparison";
            expr = Ok(Expression::LexGeq(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        "in" => {
            description =
                "membership test: verifies that the left operand belongs to the right operand";
            expr = Ok(Expression::In(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        "subset" => {
            description =
                "proper subset test: left operand must be contained in but not equal to the right";
            expr = Ok(Expression::Subset(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        "subsetEq" => {
            description =
                "subset-or-equal test: left operand is contained in or equal to the right";
            expr = Ok(Expression::SubsetEq(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        "supset" => {
            description = "proper superset test: left operand strictly contains the right";
            expr = Ok(Expression::Supset(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        "supsetEq" => {
            description = "superset-or-equal test: left operand contains or equals the right";
            expr = Ok(Expression::SupsetEq(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        "union" => {
            description = "set union: combines the elements from both operands";
            expr = Ok(Expression::Union(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        "intersect" => {
            description = "set intersection: keeps only elements common to both operands";
            expr = Ok(Expression::Intersect(
                Metadata::new(),
                Moo::new(left),
                Moo::new(right),
            ));
        }
        _ => {
            expr = Err(EssenceParseError::syntax_error(
                format!("Invalid operator: '{op_str}'"),
                Some(op_node.range()),
            ));
        }
    };

    let hover = HoverInfo {
        description: description.to_string(),
        kind: Some(SymbolKind::Function),
        ty: None,
        decl_span: None,
    };
    span_with_hover(&op_node, source_code, source_map, hover);

    expr
}

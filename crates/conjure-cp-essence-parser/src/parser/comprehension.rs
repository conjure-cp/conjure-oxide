use crate::expression::parse_expression;
use crate::parser::domain::parse_domain;
use crate::util::named_children;
use crate::{EssenceParseError, field};
use conjure_cp_core::ast::ac_operators::ACOperatorKind;
use conjure_cp_core::ast::comprehension::ComprehensionBuilder;
use conjure_cp_core::ast::{DeclarationPtr, Expression, Metadata, Moo, Name, SymbolTablePtr};
use std::vec;
use tree_sitter::Node;

pub fn parse_comprehension(
    node: &Node,
    source_code: &str,
    root: &Node,
    symbols_ptr: Option<SymbolTablePtr>,
) -> Result<Expression, EssenceParseError> {
    // Comprehensions require a symbol table passed in
    let symbols_ptr = symbols_ptr.ok_or_else(|| {
        EssenceParseError::syntax_error(
            "Comprehensions require a symbol table".to_string(),
            Some(node.range()),
        )
    })?;

    let mut builder = ComprehensionBuilder::new(symbols_ptr.clone());

    // We need to track the return expression node separately since it appears first in syntax
    // but we need to parse generators first (to get variables in scope)
    let mut return_expr_node: Option<Node> = None;

    // set return expression node and parse generators/conditions
    for child in named_children(node) {
        match child.kind() {
            "arithmetic_expr" | "bool_expr" | "comparison_expr" => {
                // Store the return expression node to parse later
                return_expr_node = Some(child);
            }
            "generator" => {
                // Parse the generator variable
                let var_node = field!(child, "variable");
                let var_name_str = &source_code[var_node.start_byte()..var_node.end_byte()];
                let var_name = Name::user(var_name_str);

                // Parse the domain
                let domain_node = field!(child, "domain");
                let var_domain = parse_domain(domain_node, source_code, Some(symbols_ptr.clone()))?;

                // Add generator using the builder
                let decl = DeclarationPtr::new_var(var_name, var_domain);
                builder = builder.generator(decl);
            }
            "condition" => {
                // Parse the condition expression
                let expr_node = field!(child, "expression");
                let generator_symboltable = builder.generator_symboltable();

                let guard_expr =
                    parse_expression(expr_node, source_code, root, Some(generator_symboltable))?;

                // Add the condition as a guard
                builder = builder.guard(guard_expr);
            }
            _ => {
                // Skip other nodes (like punctuation)
            }
        }
    }

    // parse the return expression
    let return_expr_node = return_expr_node.ok_or_else(|| {
        EssenceParseError::syntax_error(
            "Comprehension missing return expression".to_string(),
            Some(node.range()),
        )
    })?;

    // Use the return expression symbol table which already has induction variables (as Given) and parent as parent
    let return_expr = parse_expression(
        return_expr_node,
        source_code,
        root,
        Some(builder.return_expr_symboltable()),
    )?;

    // Build the comprehension with the return expression and default ACOperatorKind::And
    let comprehension = builder.with_return_value(return_expr, Some(ACOperatorKind::And));

    Ok(Expression::Comprehension(
        Metadata::new(),
        Moo::new(comprehension),
    ))
}

/// Parse comprehension-style expressions
/// - `forAll vars : domain . expr` → `And(Comprehension(...))`
/// - `sum vars : domain . expr` → `Sum(Comprehension(...))`
pub fn parse_quantifier_or_aggregate_expr(
    node: &Node,
    source_code: &str,
    root: &Node,
    symbols_ptr: Option<SymbolTablePtr>,
) -> Result<Expression, EssenceParseError> {
    // Quantifier and aggregate expressions require a symbol table
    let symbols_ptr = symbols_ptr.ok_or_else(|| {
        EssenceParseError::syntax_error(
            "Quantifier and aggregate expressions require a symbol table".to_string(),
            Some(node.range()),
        )
    })?;

    // Create the comprehension builder
    let mut builder = ComprehensionBuilder::new(symbols_ptr.clone());

    // First pass: collect domain/collection, variables
    let mut domain = None;
    let mut collection_node = None;
    let mut variables = vec![];

    for child in named_children(node) {
        match child.kind() {
            "identifier" => {
                let var_name_str = &source_code[child.start_byte()..child.end_byte()];
                let var_name = Name::user(var_name_str);
                variables.push(var_name);
            }
            "domain" => {
                domain = Some(parse_domain(child, source_code, Some(symbols_ptr.clone()))?);
            }
            "set_literal" | "matrix" | "tuple" | "record" => {
                // Store the collection node to parse later
                collection_node = Some(child);
            }
            _ => continue,
        }
    }

    // We need either a domain or a collection
    if domain.is_none() && collection_node.is_none() {
        return Err(EssenceParseError::syntax_error(
            "Quantifier and aggregate expressions require a domain or collection".to_string(),
            Some(node.range()),
        ));
    }

    if variables.is_empty() {
        return Err(EssenceParseError::syntax_error(
            "Quantifier and aggregate expressions require variables".to_string(),
            Some(node.range()),
        ));
    }

    // Get the operator type
    let operator_node = field!(node, "operator");
    let operator_str = &source_code[operator_node.start_byte()..operator_node.end_byte()];

    let (ac_operator_kind, wrapper) = match operator_str {
        "forAll" => (ACOperatorKind::And, "And"),
        "exists" => (ACOperatorKind::Or, "Or"),
        "sum" => (ACOperatorKind::Sum, "Sum"),
        "min" => (ACOperatorKind::Sum, "Min"), // AC operator doesn't matter for non-boolean aggregates
        "max" => (ACOperatorKind::Sum, "Max"),
        _ => {
            return Err(EssenceParseError::syntax_error(
                format!("Unknown operator: {}", operator_str),
                Some(operator_node.range()),
            ));
        }
    };

    // Add variables as generators
    if let Some(dom) = domain {
        for var_name in variables {
            let decl = DeclarationPtr::new_var(var_name, dom.clone());
            builder = builder.generator(decl);
        }
    } else if let Some(_coll_node) = collection_node {
        // TODO: support collection domains
        return Err(EssenceParseError::syntax_error(
            "Collection domains in quantifier and aggregate expressions not yet supported"
                .to_string(),
            Some(node.range()),
        ));
    }

    // Parse the expression (after variables are in the symbol table)
    let expression_node = node.child_by_field_name("expression").ok_or_else(|| {
        EssenceParseError::syntax_error(
            "Quantifier or aggregate expression missing return expression".to_string(),
            Some(node.range()),
        )
    })?;
    let expression = parse_expression(
        expression_node,
        source_code,
        root,
        Some(builder.return_expr_symboltable()),
    )?;

    // Build the comprehension
    let comprehension = builder.with_return_value(expression, Some(ac_operator_kind));
    let wrapped_comprehension = Expression::Comprehension(Metadata::new(), Moo::new(comprehension));

    // Wrap in the appropriate expression type
    match wrapper {
        "And" => Ok(Expression::And(
            Metadata::new(),
            Moo::new(wrapped_comprehension),
        )),
        "Or" => Ok(Expression::Or(
            Metadata::new(),
            Moo::new(wrapped_comprehension),
        )),
        "Sum" => Ok(Expression::Sum(
            Metadata::new(),
            Moo::new(wrapped_comprehension),
        )),
        "Min" => Ok(Expression::Min(
            Metadata::new(),
            Moo::new(wrapped_comprehension),
        )),
        "Max" => Ok(Expression::Max(
            Metadata::new(),
            Moo::new(wrapped_comprehension),
        )),
        _ => unreachable!(),
    }
}

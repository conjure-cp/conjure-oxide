use crate::errors::FatalParseError;
use crate::expression::parse_expression;
use crate::parser::ParseContext;
use crate::parser::domain::parse_domain;
use crate::util::named_children;
use crate::{RecoverableParseError, field};
use conjure_cp_core::ast::ac_operators::ACOperatorKind;
use conjure_cp_core::ast::comprehension::ComprehensionBuilder;
use conjure_cp_core::ast::{DeclarationPtr, Expression, Metadata, Moo, Name};
use std::vec;
use tree_sitter::Node;

pub fn parse_comprehension(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    // Comprehensions require a symbol table passed in
    let symbols_ptr = match ctx.symbols.clone() {
        Some(s) => s,
        None => {
            ctx.record_error(RecoverableParseError::new(
                "Comprehensions require a symbol table".to_string(),
                Some(node.range()),
            ));
            return Ok(None);
        }
    };

    let mut builder = ComprehensionBuilder::new(symbols_ptr);

    // We need to track the return expression node separately since it appears first in syntax
    // but we need to parse generators first (to get variables in scope)
    let mut return_expr_node: Option<Node> = None;

    // set return expression node and parse generators/conditions
    for child in named_children(node) {
        match child.kind() {
            "arithmetic_expr" | "bool_expr" | "comparison_expr" | "atom" => {
                // Store the return expression node to parse later
                return_expr_node = Some(child);
            }
            "generator" => {
                // Parse the generator variable
                let var_node = field!(child, "variable");
                let var_name_str = &ctx.source_code[var_node.start_byte()..var_node.end_byte()];
                let var_name = Name::user(var_name_str);

                // Parse the domain
                let domain_node = field!(child, "domain");

                // Parse with a new context using the generator symbol table
                let mut domain_ctx = ctx.with_new_symbols(Some(builder.generator_symboltable()));
                let Some(var_domain) = parse_domain(&mut domain_ctx, domain_node)? else {
                    return Ok(None);
                };

                // Add generator using the builder
                let decl = DeclarationPtr::new_find(var_name, var_domain);
                builder = builder.generator(decl);
            }
            "condition" => {
                // Parse the condition expression
                let expr_node = field!(child, "expression");
                let generator_symboltable = builder.generator_symboltable();

                // Parse with a new context using the generator symbol table
                let mut guard_ctx = ctx.with_new_symbols(Some(generator_symboltable));
                let Some(guard_expr) = parse_expression(&mut guard_ctx, expr_node)? else {
                    return Ok(None);
                };

                // Add the condition as a guard
                builder = builder.guard(guard_expr);
            }
            _ => {
                // Skip other nodes (like punctuation)
            }
        }
    }

    // parse the return expression
    let return_expr_node = match return_expr_node {
        Some(node) => node,
        None => {
            ctx.record_error(RecoverableParseError::new(
                "Comprehension missing return expression".to_string(),
                Some(node.range()),
            ));
            return Ok(None);
        }
    };

    // Use the return expression symbol table which already has quantified variables (as Given) and parent as parent
    let mut return_ctx = ctx.with_new_symbols(Some(builder.return_expr_symboltable()));
    let Some(return_expr) = parse_expression(&mut return_ctx, return_expr_node)? else {
        return Ok(None);
    };

    // Build the comprehension with the return expression and default ACOperatorKind::And
    let comprehension = builder.with_return_value(return_expr, Some(ACOperatorKind::And));

    Ok(Some(Expression::Comprehension(
        Metadata::new(),
        Moo::new(comprehension),
    )))
}

/// Parse comprehension-style expressions
/// - `forAll vars : domain . expr` → `And(Comprehension(...))`
/// - `sum vars : domain . expr` → `Sum(Comprehension(...))`
pub fn parse_quantifier_or_aggregate_expr(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    // Quantifier and aggregate expressions require a symbol table
    let symbols_ptr = match ctx.symbols.clone() {
        Some(s) => s,
        None => {
            ctx.record_error(RecoverableParseError::new(
                "Quantifier and aggregate expressions require a symbol table".to_string(),
                Some(node.range()),
            ));
            return Ok(None);
        }
    };

    // Create the comprehension builder
    let mut builder = ComprehensionBuilder::new(symbols_ptr);

    // First pass: collect domain/collection, variables
    let mut domain = None;
    let mut collection_node = None;
    let mut variables = vec![];

    for child in named_children(node) {
        match child.kind() {
            "identifier" => {
                let var_name_str = &ctx.source_code[child.start_byte()..child.end_byte()];
                let var_name = Name::user(var_name_str);
                variables.push(var_name);
            }
            "domain" => {
                // Parse with the current symbol table (no need for a new context)
                let Some(parsed_domain) = parse_domain(ctx, child)? else {
                    return Ok(None);
                };
                domain = Some(parsed_domain);
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
        ctx.record_error(RecoverableParseError::new(
            "Quantifier and aggregate expressions require a domain or collection".to_string(),
            Some(node.range()),
        ));
        return Ok(None);
    }

    if variables.is_empty() {
        ctx.record_error(RecoverableParseError::new(
            "Quantifier and aggregate expressions require variables".to_string(),
            Some(node.range()),
        ));
        return Ok(None);
    }

    // Get the operator type
    let operator_node = field!(node, "operator");
    let operator_str = &ctx.source_code[operator_node.start_byte()..operator_node.end_byte()];

    let (ac_operator_kind, wrapper) = match operator_str {
        "forAll" => (ACOperatorKind::And, "And"),
        "exists" => (ACOperatorKind::Or, "Or"),
        "sum" => (ACOperatorKind::Sum, "Sum"),
        "min" => (ACOperatorKind::Sum, "Min"), // AC operator doesn't matter for non-boolean aggregates
        "max" => (ACOperatorKind::Sum, "Max"),
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Unknown operator: {}", operator_str),
                Some(operator_node.range()),
            ));
            return Ok(None);
        }
    };

    // Add variables as generators
    if let Some(dom) = domain {
        for var_name in variables {
            let decl = DeclarationPtr::new_find(var_name, dom.clone());
            builder = builder.generator(decl);
        }
    } else if let Some(_coll_node) = collection_node {
        // TODO: support collection domains
        ctx.record_error(RecoverableParseError::new(
            "Collection domains in quantifier and aggregate expressions".to_string(),
            Some(_coll_node.range()),
        ));
        return Ok(None);
    }

    // Parse the expression (after variables are in the symbol table)
    let expression_node = field!(node, "expression");

    // Parse with a new context using the return expression symbol table
    let mut expr_ctx = ctx.with_new_symbols(Some(builder.return_expr_symboltable()));
    let Some(expression) = parse_expression(&mut expr_ctx, expression_node)? else {
        return Ok(None);
    };

    // Build the comprehension
    let comprehension = builder.with_return_value(expression, Some(ac_operator_kind));
    let wrapped_comprehension = Expression::Comprehension(Metadata::new(), Moo::new(comprehension));

    // Wrap in the appropriate expression type
    match wrapper {
        "And" => Ok(Some(Expression::And(
            Metadata::new(),
            Moo::new(wrapped_comprehension),
        ))),
        "Or" => Ok(Some(Expression::Or(
            Metadata::new(),
            Moo::new(wrapped_comprehension),
        ))),
        "Sum" => Ok(Some(Expression::Sum(
            Metadata::new(),
            Moo::new(wrapped_comprehension),
        ))),
        "Min" => Ok(Some(Expression::Min(
            Metadata::new(),
            Moo::new(wrapped_comprehension),
        ))),
        "Max" => Ok(Some(Expression::Max(
            Metadata::new(),
            Moo::new(wrapped_comprehension),
        ))),
        _ => unreachable!(),
    }
}

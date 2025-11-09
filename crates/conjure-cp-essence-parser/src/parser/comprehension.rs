use crate::expression::parse_expression;
use crate::parser::domain::parse_domain;
use crate::util::named_children;
use crate::{EssenceParseError, field};
use conjure_cp_core::ast::ac_operators::ACOperatorKind;
use conjure_cp_core::ast::comprehension::ComprehensionBuilder;
use conjure_cp_core::ast::{DeclarationPtr, Expression, Metadata, Moo, Name, SymbolTable};
use std::cell::RefCell;
use std::rc::Rc;
use tree_sitter::Node;

pub fn parse_comprehension(
    node: &Node,
    source_code: &str,
    root: &Node,
    symbols_ptr: Option<Rc<RefCell<SymbolTable>>>,
) -> Result<Expression, EssenceParseError> {
    // Comprehensions require a symbol table passed in
    let symbols_ptr = symbols_ptr.ok_or_else(|| {
        EssenceParseError::syntax_error(
            "Comprehensions require a symbol table".to_string(),
            Some(node.range()),
        )
    })?;

    let mut builder = ComprehensionBuilder::new(symbols_ptr);

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
                let var_domain = parse_domain(domain_node, source_code)?;

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

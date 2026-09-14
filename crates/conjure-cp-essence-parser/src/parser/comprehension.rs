use crate::RecoverableParseError;
use crate::errors::FatalParseError;
use crate::expression::parse_expression;
use crate::field;
use crate::parser::ParseContext;
use crate::parser::atom::parse_atom;
use crate::parser::domain::parse_domain;
use crate::util::{TypecheckingContext, named_children};
use conjure_cp_core::ast::ac_operators::ACOperatorKind;
use conjure_cp_core::ast::comprehension::ComprehensionBuilder;
use conjure_cp_core::ast::serde::HasId as _;
use conjure_cp_core::ast::{
    Atom, DeclarationPtr, Expression, Literal, Metadata, Moo, Name, Reference,
};
use tree_sitter::Node;
use uniplate::Uniplate as _;

fn parse_collection_expression(
    ctx: &mut ParseContext,
    collection_node: Node,
) -> Result<Option<Expression>, FatalParseError> {
    match collection_node.kind() {
        "arithmetic_expr" | "bool_expr" | "comparison_expr" => {
            parse_expression(ctx, collection_node)
        }
        "atom" | "matrix" | "set_literal" | "tuple" | "record" | "identifier"
        | "index_or_slice" | "set_operation" => parse_atom(ctx, &collection_node),
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Unexpected collection type: '{}'", collection_node.kind()),
                Some(collection_node.range()),
            ));
            Ok(None)
        }
    }
}

/// Collects the names a generator pattern binds, each with its index path into the generator
/// variable: `(i, _)` binds `i` at path `[1]`, and `(i, (j, k))` binds `k` at path `[2, 2]`.
///
/// Patterns are written as tuples, so an element arrives wrapped in whatever expression nodes a
/// tuple element goes through; only the identifiers at the bottom matter here.
///
/// Returns false for a pattern this cannot read, having recorded an error.
fn collect_pattern_bindings(
    ctx: &mut ParseContext,
    node: &Node,
    path: &mut Vec<i32>,
    bindings: &mut Vec<(Name, Vec<i32>)>,
) -> bool {
    match node.kind() {
        "identifier" => {
            let name = &ctx.source_code[node.start_byte()..node.end_byte()];
            // `_` names a component the pattern discards.
            if name != "_" {
                bindings.push((Name::user(name), path.clone()));
            }
            true
        }
        "tuple" => {
            for (position, element) in named_children(node).enumerate() {
                path.push(position as i32 + 1);
                let bound = collect_pattern_bindings(ctx, &element, path, bindings);
                path.pop();
                if !bound {
                    return false;
                }
            }
            true
        }
        // A tuple element is wrapped in expression nodes; unwrap to what it actually holds.
        _ => match named_children(node).collect::<Vec<_>>().as_slice() {
            [inner] => collect_pattern_bindings(ctx, inner, path, bindings),
            _ => {
                ctx.record_error(RecoverableParseError::new(
                    format!(
                        "Expected a name or `_` in a generator pattern, got '{}'",
                        &ctx.source_code[node.start_byte()..node.end_byte()]
                    ),
                    Some(node.range()),
                ));
                false
            }
        },
    }
}

/// Binds the names a generator pattern introduces, for a generator already added to `builder`
/// under `var_name`.
///
/// Patterns are handled exactly like comprehension lettings: each name is bound to a projection
/// out of the generator variable and substituted into everything that follows, so nothing
/// downstream has to know that a pattern was written.
fn bind_generator_pattern(
    ctx: &mut ParseContext,
    builder: &mut ComprehensionBuilder,
    var_node: &Node,
    var_name: &Name,
    lettings: &mut Vec<(DeclarationPtr, Expression)>,
) -> bool {
    if var_node.kind() != "tuple" {
        return true;
    }

    let mut bindings = Vec::new();
    if !collect_pattern_bindings(ctx, var_node, &mut Vec::new(), &mut bindings) {
        return false;
    }

    let symbols = builder.generator_symboltable();
    let Some(generator_decl) = symbols.read().lookup_local(var_name) else {
        ctx.record_error(RecoverableParseError::new(
            format!("Generator pattern variable {var_name} is not in scope"),
            Some(var_node.range()),
        ));
        return false;
    };

    for (name, path) in bindings {
        let mut value = Expression::from(Reference::new(generator_decl.clone()));
        for index in path {
            value = Expression::SafeIndex(
                Metadata::new(),
                Moo::new(value),
                vec![Expression::from(Literal::Int(index))],
            );
        }
        let decl = DeclarationPtr::new_value_letting(name, value.clone());
        symbols.write().insert(decl.clone());
        lettings.push((decl, value));
    }

    true
}

fn parse_generator(
    ctx: &mut ParseContext,
    mut builder: ComprehensionBuilder,
    generator_node: &Node,
    lettings: &mut Vec<(DeclarationPtr, Expression)>,
) -> Result<Option<ComprehensionBuilder>, FatalParseError> {
    let Some(var_node) = field!(recover, ctx, generator_node, "variable") else {
        return Ok(None);
    };
    let var_name_str = &ctx.source_code[var_node.start_byte()..var_node.end_byte()];
    let var_name = Name::user(var_name_str);

    if let Some(domain_node) = generator_node.child_by_field_name("domain") {
        let mut domain_ctx = ctx.with_new_symbols(Some(builder.generator_symboltable()));
        let Some(var_domain) = parse_domain(&mut domain_ctx, domain_node)? else {
            return Ok(None);
        };
        let decl = DeclarationPtr::new_find(var_name.clone(), var_domain);
        let mut builder = builder.generator(decl);
        if !bind_generator_pattern(ctx, &mut builder, &var_node, &var_name, lettings) {
            return Ok(None);
        }
        return Ok(Some(builder));
    }

    if let Some(collection_node) = generator_node.child_by_field_name("collection") {
        let mut collection_ctx = ctx.with_new_symbols(Some(builder.generator_symboltable()));
        collection_ctx.typechecking_context = TypecheckingContext::Unknown;
        collection_ctx.inner_typechecking_context = TypecheckingContext::Unknown;
        let Some(collection_expr) =
            parse_collection_expression(&mut collection_ctx, collection_node)?
        else {
            return Ok(None);
        };
        let collection_expr = substitute_lettings(collection_expr, lettings);
        let mut builder = builder.expression_generator(var_name.clone(), collection_expr);
        if !bind_generator_pattern(ctx, &mut builder, &var_node, &var_name, lettings) {
            return Ok(None);
        }
        return Ok(Some(builder));
    }

    ctx.record_error(RecoverableParseError::new(
        format!(
            "Generator requires either a domain or collection (e.g. `{var_name} : domain` or `{var_name} <- expr`)"
        ),
        Some(generator_node.range()),
    ));
    Ok(None)
}

fn parse_quantifier_variables<'a>(
    ctx: &mut ParseContext,
    node: &Node<'a>,
) -> Option<Vec<(Name, Node<'a>)>> {
    let mut variables = Vec::new();
    let mut cursor = node.walk();
    for child in node.children_by_field_name("variables", &mut cursor) {
        if matches!(child.kind(), "identifier" | "tuple") {
            let var_name_str = &ctx.source_code[child.start_byte()..child.end_byte()];
            variables.push((Name::user(var_name_str), child));
        }
    }

    if variables.is_empty() {
        ctx.record_error(RecoverableParseError::new(
            "Quantifier and aggregate expressions require variables".to_string(),
            Some(node.range()),
        ));
        return None;
    }

    Some(variables)
}

fn add_quantifier_generators_from_collection(
    ctx: &mut ParseContext,
    mut builder: ComprehensionBuilder,
    variables: &[(Name, Node)],
    collection_node: Node,
    lettings: &mut Vec<(DeclarationPtr, Expression)>,
) -> Result<Option<ComprehensionBuilder>, FatalParseError> {
    let mut collection_ctx = ctx.with_new_symbols(Some(builder.generator_symboltable()));
    collection_ctx.typechecking_context = TypecheckingContext::Unknown;
    collection_ctx.inner_typechecking_context = TypecheckingContext::Unknown;
    let Some(collection_expr) = parse_collection_expression(&mut collection_ctx, collection_node)?
    else {
        return Ok(None);
    };

    for (var_name, var_node) in variables {
        builder = builder.expression_generator(var_name.clone(), collection_expr.clone());
        if !bind_generator_pattern(ctx, &mut builder, var_node, var_name, lettings) {
            return Ok(None);
        }
    }

    Ok(Some(builder))
}

/// Replaces references to comprehension lettings with the expressions they are bound to.
///
/// Comprehension lettings are pure sugar. They cannot be left as declarations for the rewriter:
/// their value usually mentions the generator variables, which comprehension expansion substitutes
/// in the comprehension body but not in declarations stored in the symbol table.
fn substitute_lettings(expr: Expression, lettings: &[(DeclarationPtr, Expression)]) -> Expression {
    if lettings.is_empty() {
        return expr;
    }

    expr.transform(&|expr| match &expr {
        Expression::Atomic(_, Atom::Reference(reference)) => lettings
            .iter()
            .find(|(decl, _)| decl.id() == reference.id())
            .map(|(_, value)| value.clone())
            .unwrap_or(expr),
        _ => expr,
    })
}

pub fn parse_comprehension(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    // If we're in a set context, add error and return early since comprehensions don't produce sets
    if ctx.typechecking_context == crate::util::TypecheckingContext::Set {
        ctx.record_error(crate::errors::RecoverableParseError::new(
            format!(
                "Type error: {}\n\tExpected: set\n\tGot: comprehension",
                ctx.source_code[node.start_byte()..node.end_byte()].trim()
            ),
            Some(node.range()),
        ));
    }

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

    // Names bound by `letting` qualifiers, substituted back into everything that follows.
    let mut lettings: Vec<(DeclarationPtr, Expression)> = Vec::new();

    // set return expression node and parse generators/conditions
    for child in named_children(node) {
        match child.kind() {
            "arithmetic_expr" | "bool_expr" | "comparison_expr" | "atom" => {
                // Store the return expression node to parse later
                return_expr_node = Some(child);
            }
            "generator" => {
                let Some(updated_builder) = parse_generator(ctx, builder, &child, &mut lettings)?
                else {
                    return Ok(None);
                };
                builder = updated_builder;
            }
            "condition" => {
                // Parse the condition expression
                let Some(expr_node) = field!(recover, ctx, child, "expression") else {
                    return Ok(None);
                };
                let generator_symboltable = builder.generator_symboltable();

                // Parse with a new context using the generator symbol table
                let mut guard_ctx = ctx.with_new_symbols(Some(generator_symboltable));
                let Some(guard_expr) = parse_expression(&mut guard_ctx, expr_node)? else {
                    return Ok(None);
                };

                // Add the condition as a guard
                builder = builder.guard(substitute_lettings(guard_expr, &lettings));
            }
            "comprehension_letting" => {
                let Some(name_node) = field!(recover, ctx, child, "name") else {
                    return Ok(None);
                };
                let Some(value_node) = field!(recover, ctx, child, "value") else {
                    return Ok(None);
                };

                let generator_symboltable = builder.generator_symboltable();
                let mut letting_ctx = ctx.with_new_symbols(Some(generator_symboltable.clone()));
                letting_ctx.typechecking_context = TypecheckingContext::Unknown;
                letting_ctx.inner_typechecking_context = TypecheckingContext::Unknown;
                let Some(value) = parse_expression(&mut letting_ctx, value_node)? else {
                    return Ok(None);
                };
                let value = substitute_lettings(value, &lettings);

                let name =
                    Name::user(&ctx.source_code[name_node.start_byte()..name_node.end_byte()]);
                // The declaration only exists so that the name resolves while parsing what
                // follows; every reference to it is substituted away before the comprehension is
                // built.
                let decl = DeclarationPtr::new_value_letting(name, value.clone());
                generator_symboltable.write().insert(decl.clone());
                lettings.push((decl, value));
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
    // Parse using the inner typechecking context
    let saved_inner_ctx = ctx.inner_typechecking_context;
    let mut return_ctx = ctx.with_new_symbols(Some(builder.return_expr_symboltable()));
    return_ctx.typechecking_context = saved_inner_ctx;
    return_ctx.inner_typechecking_context = TypecheckingContext::Unknown;
    let Some(return_expr) = parse_expression(&mut return_ctx, return_expr_node)? else {
        return Ok(None);
    };

    // Build the comprehension with the return expression
    let comprehension = builder.with_return_value(substitute_lettings(return_expr, &lettings));

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

    // Names bound by generator patterns, substituted back into everything that follows.
    let mut lettings: Vec<(DeclarationPtr, Expression)> = Vec::new();

    let Some(variables) = parse_quantifier_variables(ctx, node) else {
        return Ok(None);
    };

    let domain_node = node.child_by_field_name("domain");
    let collection_node = node.child_by_field_name("collection");

    if domain_node.is_none() && collection_node.is_none() {
        ctx.record_error(RecoverableParseError::new(
            "Quantifier and aggregate expressions require a domain or collection".to_string(),
            Some(node.range()),
        ));
        return Ok(None);
    }

    if domain_node.is_some() && collection_node.is_some() {
        ctx.record_error(RecoverableParseError::new(
            "Quantifier and aggregate expressions cannot have both a domain and a collection"
                .to_string(),
            Some(node.range()),
        ));
        return Ok(None);
    }

    // Get the operator type
    let Some(operator_node) = field!(recover, ctx, node, "operator") else {
        return Ok(None);
    };
    let operator_str = &ctx.source_code[operator_node.start_byte()..operator_node.end_byte()];

    let (ac_operator_kind, wrapper) = match operator_str {
        "forAll" => (ACOperatorKind::And, "And"),
        "exists" => (ACOperatorKind::Or, "Or"),
        "sum" => (ACOperatorKind::Sum, "Sum"),
        // min/max are not true AC operators, but still need their own skip-operator tag so a
        // symbolic guard lowers correctly instead of silently substituting Sum's identity (0).
        "min" => (ACOperatorKind::Min, "Min"),
        "max" => (ACOperatorKind::Max, "Max"),
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Unknown operator: {}", operator_str),
                Some(operator_node.range()),
            ));
            return Ok(None);
        }
    };

    // Add variables as generators
    if let Some(domain_node) = domain_node {
        let saved_ctx = ctx.typechecking_context;
        let saved_inner_ctx = ctx.inner_typechecking_context;
        ctx.typechecking_context = TypecheckingContext::Unknown;
        ctx.inner_typechecking_context = TypecheckingContext::Unknown;

        let Some(domain) = parse_domain(ctx, domain_node)? else {
            ctx.typechecking_context = saved_ctx;
            ctx.inner_typechecking_context = saved_inner_ctx;
            return Ok(None);
        };

        ctx.typechecking_context = saved_ctx;
        ctx.inner_typechecking_context = saved_inner_ctx;

        for (var_name, var_node) in &variables {
            let decl = DeclarationPtr::new_find(var_name.clone(), domain.clone());
            builder = builder.generator(decl);
            if !bind_generator_pattern(ctx, &mut builder, var_node, var_name, &mut lettings) {
                return Ok(None);
            }
        }
    } else if let Some(collection_node) = collection_node {
        let Some(updated_builder) = add_quantifier_generators_from_collection(
            ctx,
            builder,
            &variables,
            collection_node,
            &mut lettings,
        )?
        else {
            return Ok(None);
        };
        builder = updated_builder;
    }

    // A guard written between the generator and the `.`, e.g. `forAll (i, _) in s, i > 1 . ...`.
    if let Some(guard_node) = node.child_by_field_name("guard") {
        let mut guard_ctx = ctx.with_new_symbols(Some(builder.generator_symboltable()));
        guard_ctx.typechecking_context = TypecheckingContext::Unknown;
        guard_ctx.inner_typechecking_context = TypecheckingContext::Unknown;
        let Some(guard) = parse_expression(&mut guard_ctx, guard_node)? else {
            return Ok(None);
        };
        builder = builder.guard(substitute_lettings(guard, &lettings));
    }

    // Parse the expression (after variables are in the symbol table)
    let Some(expression_node) = field!(recover, ctx, node, "expression") else {
        return Ok(None);
    };

    // Parse with a new context using the return expression symbol table
    // Prase using the inner typechecking context
    let saved_inner_ctx = ctx.inner_typechecking_context;
    let mut expr_ctx = ctx.with_new_symbols(Some(builder.return_expr_symboltable()));
    expr_ctx.typechecking_context = saved_inner_ctx;
    expr_ctx.inner_typechecking_context = TypecheckingContext::Unknown;
    let Some(expression) = parse_expression(&mut expr_ctx, expression_node)? else {
        return Ok(None);
    };

    // Build the comprehension
    let mut comprehension = builder.with_return_value(substitute_lettings(expression, &lettings));
    comprehension.skip_operator = Some(ac_operator_kind);
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

use std::collections::VecDeque;

use crate::diagnostics::diagnostics_api::SymbolKind;
use crate::diagnostics::source_map::{HoverInfo, span_with_hover};
use crate::errors::{FatalParseError, RecoverableParseError};
use crate::expression::{parse_annotation_expression, parse_binary_expression, parse_expression};
use crate::parser::ParseContext;
use crate::parser::abstract_literal::parse_abstract;
use crate::parser::comprehension::parse_comprehension;
use crate::parser::domain::parse_domain;
use crate::parser::dominance::parse_pareto_expression;
use crate::parser::global_constraints::ELEMENT_ID;
use crate::util::{TypecheckingContext, named_children};
use crate::{field, named_child};

use conjure_cp_core::ast::{
    Atom, DeclarationKind, DeclarationPtr, Expression, GroundDomain, Literal, Metadata, Moo, Name,
    ReturnType, Typeable,
};

use tree_sitter::Node;
use ustr::Ustr;

pub fn parse_atom(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    match node.kind() {
        "atom" | "primary_atom" | "sub_atom_expr" => {
            let Some(inner) = named_child!(recover, ctx, node) else {
                return Ok(None);
            };
            parse_atom(ctx, &inner)
        }
        "metavar" => {
            let Some(ident) = field!(recover, ctx, node, "identifier") else {
                return Ok(None);
            };
            let name_str = &ctx.source_code[ident.start_byte()..ident.end_byte()];
            Ok(Some(Expression::Metavar(
                Metadata::new(),
                Ustr::from(name_str),
            )))
        }
        "identifier" => {
            let Some(var) = parse_variable(ctx, node)? else {
                return Ok(None);
            };
            Ok(Some(Expression::Atomic(Metadata::new(), var)))
        }
        "from_solution" => {
            if ctx.root.kind() != "dominance_relation" {
                ctx.record_error(RecoverableParseError::new(
                    "fromSolution only allowed inside dominance relations".to_string(),
                    Some(node.range()),
                ));
                return Ok(None);
            }

            let Some(var_node) = field!(recover, ctx, node, "variable") else {
                return Ok(None);
            };
            let Some(inner) = parse_variable(ctx, &var_node)? else {
                return Ok(None);
            };

            Ok(Some(Expression::FromSolution(
                Metadata::new(),
                Moo::new(inner),
            )))
        }
        "pareto_expression" => parse_pareto_expression(ctx, node),
        "constant" => {
            let Some(lit) = parse_constant(ctx, node)? else {
                return Ok(None);
            };
            Ok(Some(Expression::Atomic(
                Metadata::new(),
                Atom::Literal(lit),
            )))
        }
        "matrix" | "record" | "variant" | "tuple" | "set_literal" | "mset_literal"
        | "sequence_literal" | "function_literal" | "relation_literal" | "partition_literal"
        | "permutation_literal" => {
            let Some(abs) = parse_abstract(ctx, node)? else {
                return Ok(None);
            };
            Ok(Some(Expression::AbstractLiteral(Metadata::new(), abs)))
        }
        "flatten" => parse_flatten(ctx, node),
        "element_id" => parse_element_id(ctx, node),
        "table" | "negative_table" => parse_table(ctx, node),
        "index_or_slice" => parse_index_or_slice(ctx, node),
        "annotation_expr" => parse_annotation_expression(ctx, node),
        // for now, assume is binary since powerset isn't implemented
        // TODO: add powerset support under "set_operation"
        "set_operation" => parse_binary_expression(ctx, node),
        "comprehension" => parse_comprehension(ctx, node),
        "defined_expr" | "range_expr" | "to_set_expr" | "to_mset_expr" | "to_relation_expr"
        | "perm_inverse_expr" => parse_function_unary_operator(ctx, node),
        "image_expr" | "image_set_expr" | "pre_image_expr" | "inverse_expr" | "compose_expr" => {
            parse_function_binary_operator(ctx, node)
        }
        "restrict_expr" => parse_restrict_expr(ctx, node),
        "attribute_as_constraint_expr" => parse_attribute_as_constraint(ctx, node),
        "parts_expr" | "participants_expr" => parse_partition_unary_operator(ctx, node),
        "together_expr" | "apart_expr" | "party_expr" => parse_partition_binary_operator(ctx, node),
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Expected atom, got: {}", node.kind()),
                Some(node.range()),
            ));
            Ok(None)
        }
    }
}

fn parse_flatten(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    // add error and return early if we're in a set context, since flatten doesn't produce sets
    if ctx.typechecking_context == TypecheckingContext::Set {
        ctx.record_error(RecoverableParseError::new(
            format!(
                "Type error: {}\n\tExpected: set\n\tGot: flatten",
                ctx.source_code[node.start_byte()..node.end_byte()].trim()
            ),
            Some(node.range()),
        ));
        return Ok(None);
    }

    let Some(expr_node) = field!(recover, ctx, node, "expression") else {
        return Ok(None);
    };
    // TODO: verify the atom is a matrix
    let Some(expr) = parse_atom(ctx, &expr_node)? else {
        return Ok(None);
    };

    if let Some(depth_node) = node.child_by_field_name("depth") {
        let Some(depth) = parse_int(ctx, &depth_node) else {
            return Ok(None);
        };
        let depth_expression =
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(depth)));
        Ok(Some(Expression::Flatten(
            Metadata::new(),
            Some(Moo::new(depth_expression)),
            Moo::new(expr),
        )))
    } else {
        Ok(Some(Expression::Flatten(
            Metadata::new(),
            None,
            Moo::new(expr),
        )))
    }
}

fn parse_table(ctx: &mut ParseContext, node: &Node) -> Result<Option<Expression>, FatalParseError> {
    // add error and return early if we're in a set context, since tables aren't allowed there
    if ctx.typechecking_context == TypecheckingContext::Set {
        ctx.record_error(RecoverableParseError::new(
            format!(
                "Type error: {}\n\tExpected: set\n\tGot: table",
                ctx.source_code[node.start_byte()..node.end_byte()].trim()
            ),
            Some(node.range()),
        ));
        return Ok(None);
    }

    // the variables and rows can contain arbitrary expressions, so we temporarily set the context to Unknown to avoid typechecking errors
    let saved_context = ctx.typechecking_context;
    ctx.typechecking_context = TypecheckingContext::Unknown;

    let Some(variables_node) = field!(recover, ctx, node, "variables") else {
        return Ok(None);
    };
    let Some(variables) = parse_table_operand(ctx, &variables_node)? else {
        return Ok(None);
    };

    let Some(rows_node) = field!(recover, ctx, node, "rows") else {
        return Ok(None);
    };
    let Some(rows) = parse_table_operand(ctx, &rows_node)? else {
        return Ok(None);
    };

    ctx.typechecking_context = saved_context;

    match node.kind() {
        "table" => Ok(Some(Expression::Table(
            Metadata::new(),
            Moo::new(variables),
            Moo::new(rows),
        ))),
        "negative_table" => Ok(Some(Expression::NegativeTable(
            Metadata::new(),
            Moo::new(variables),
            Moo::new(rows),
        ))),
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!(
                    "Expected 'table' or 'negative_table', got: '{}'",
                    node.kind()
                ),
                Some(node.range()),
            ));
            Ok(None)
        }
    }
}

pub fn parse_table_operand(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    match node.kind() {
        "identifier" | "constant" | "matrix" | "index_or_slice" | "flatten" => {
            parse_atom(ctx, node)
        }
        "sub_arith_expr" | "negative_expr" | "abs_value" | "factorial_expr" | "arithmetic_expr"
        | "product_expr" | "sum_expr" | "exponent" | "bool_expr" | "comparison_expr" => {
            parse_expression(ctx, *node)
        }
        _ => parse_expression(ctx, *node),
    }
}

fn parse_element_id(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    if ctx.typechecking_context == TypecheckingContext::Set {
        ctx.record_error(RecoverableParseError::new(
            format!(
                "Type error: {}\n\tExpected: set\n\tGot: elementId",
                ctx.source_code[node.start_byte()..node.end_byte()].trim()
            ),
            Some(node.range()),
        ));
        return Ok(None);
    }

    let saved_context = ctx.typechecking_context;
    ctx.typechecking_context = TypecheckingContext::Unknown;

    let Some(matrix_node) = field!(recover, ctx, node, "matrix") else {
        return Ok(None);
    };
    let Some(matrix) = parse_table_operand(ctx, &matrix_node)? else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };

    let Some(value_node) = field!(recover, ctx, node, "value") else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };
    let Some(value) = parse_table_operand(ctx, &value_node)? else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };

    ctx.typechecking_context = saved_context;

    let operator_node = node.child(0).unwrap();
    ctx.add_span_and_doc_hover(&operator_node, ELEMENT_ID, SymbolKind::Function, None, None);

    Ok(Some(Expression::ElementId(
        Metadata::new(),
        Moo::new(matrix),
        Moo::new(value),
    )))
}

/// Parses the single argument of a unary function-call-style operator (`defined`, `range`,
/// `toSet`, `toMSet`, `toRelation`): keyword immediately followed by a parenthesised argument.
/// The argument's type varies by operator (function, set, mset, relation), so typechecking
/// context is relaxed to `Unknown` while parsing it, mirroring `parse_flatten`/`parse_element_id`.
fn parse_function_unary_operator(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    let field_name = if node.kind() == "perm_inverse_expr" {
        "permutation"
    } else {
        "function"
    };

    let saved_context = ctx.typechecking_context;
    ctx.typechecking_context = TypecheckingContext::Unknown;

    let Some(function_node) = field!(recover, ctx, node, field_name) else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };
    let Some(function) = parse_expression(ctx, function_node)? else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };

    ctx.typechecking_context = saved_context;

    match node.kind() {
        "defined_expr" => Ok(Some(Expression::Defined(
            Metadata::new(),
            Moo::new(function),
        ))),
        "range_expr" => Ok(Some(Expression::Range(Metadata::new(), Moo::new(function)))),
        "to_set_expr" => Ok(Some(Expression::ToSet(Metadata::new(), Moo::new(function)))),
        "to_mset_expr" => Ok(Some(Expression::ToMSet(
            Metadata::new(),
            Moo::new(function),
        ))),
        "to_relation_expr" => Ok(Some(Expression::ToRelation(
            Metadata::new(),
            Moo::new(function),
        ))),
        "perm_inverse_expr" => Ok(Some(Expression::PermInverse(
            Metadata::new(),
            Moo::new(function),
        ))),
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Expected a function operator, got: {}", node.kind()),
                Some(node.range()),
            ));
            Ok(None)
        }
    }
}

/// Parses an attribute-as-constraint predicate, e.g. `reflexive(r)` or `size(s, 3)`: a keyword
/// naming the attribute, immediately followed by a parenthesised target and an optional value.
/// Arity is not checked here (the grammar allows any attribute name with or without a value); a
/// later pass validates it against the target's actual domain kind.
fn parse_attribute_as_constraint(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    let Some(attr_node) = field!(recover, ctx, node, "attribute") else {
        return Ok(None);
    };
    let attr = Ustr::from(&ctx.source_code[attr_node.start_byte()..attr_node.end_byte()]);

    let saved_context = ctx.typechecking_context;
    ctx.typechecking_context = TypecheckingContext::Unknown;

    let Some(target_node) = field!(recover, ctx, node, "target") else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };
    let Some(target) = parse_expression(ctx, target_node)? else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };

    let value = match node.child_by_field_name("value") {
        Some(value_node) => match parse_expression(ctx, value_node)? {
            Some(value_expr) => Some(Moo::new(value_expr)),
            None => {
                ctx.typechecking_context = saved_context;
                return Ok(None);
            }
        },
        None => None,
    };

    ctx.typechecking_context = saved_context;

    Ok(Some(Expression::AttributeAsConstraint(
        Metadata::new(),
        Moo::new(target),
        attr,
        value,
    )))
}

/// Parses a unary partition operator (`parts`, `participants`): keyword immediately followed by
/// a parenthesised partition expression.
fn parse_partition_unary_operator(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    let saved_context = ctx.typechecking_context;
    ctx.typechecking_context = TypecheckingContext::Unknown;

    let Some(partition_node) = field!(recover, ctx, node, "partition") else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };
    let Some(partition) = parse_expression(ctx, partition_node)? else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };

    ctx.typechecking_context = saved_context;

    match node.kind() {
        "parts_expr" => Ok(Some(Expression::Parts(
            Metadata::new(),
            Moo::new(partition),
        ))),
        "participants_expr" => Ok(Some(Expression::Participants(
            Metadata::new(),
            Moo::new(partition),
        ))),
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Expected a partition operator, got: {}", node.kind()),
                Some(node.range()),
            ));
            Ok(None)
        }
    }
}

/// Parses the two arguments of a binary partition operator (`together`, `apart`, `party`):
/// keyword immediately followed by a parenthesised, comma-separated argument pair. `party_expr`
/// uses an `element`/`partition` field pair; `together_expr`/`apart_expr` use `elements`/
/// `partition`.
fn parse_partition_binary_operator(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    let first_field = if node.kind() == "party_expr" {
        "element"
    } else {
        "elements"
    };

    let saved_context = ctx.typechecking_context;
    ctx.typechecking_context = TypecheckingContext::Unknown;

    let Some(first_node) = field!(recover, ctx, node, first_field) else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };
    let Some(first) = parse_expression(ctx, first_node)? else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };

    let Some(partition_node) = field!(recover, ctx, node, "partition") else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };
    let Some(partition) = parse_expression(ctx, partition_node)? else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };

    ctx.typechecking_context = saved_context;

    match node.kind() {
        "together_expr" => Ok(Some(Expression::Together(
            Metadata::new(),
            Moo::new(first),
            Moo::new(partition),
        ))),
        "apart_expr" => Ok(Some(Expression::Apart(
            Metadata::new(),
            Moo::new(first),
            Moo::new(partition),
        ))),
        "party_expr" => Ok(Some(Expression::Party(
            Metadata::new(),
            Moo::new(first),
            Moo::new(partition),
        ))),
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Expected a partition operator, got: {}", node.kind()),
                Some(node.range()),
            ));
            Ok(None)
        }
    }
}

/// Parses the two arguments of a binary function-call-style operator (`image`, `imageSet`,
/// `preImage`, `inverse`): keyword immediately followed by a parenthesised, comma-separated
/// argument pair. `inverse_expr` uses `left`/`right` fields; the rest use `function`/`argument`.
fn parse_function_binary_operator(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    let (first_field, second_field) = if matches!(node.kind(), "inverse_expr" | "compose_expr") {
        ("left", "right")
    } else {
        ("function", "argument")
    };

    let saved_context = ctx.typechecking_context;
    ctx.typechecking_context = TypecheckingContext::Unknown;

    let Some(first_node) = field!(recover, ctx, node, first_field) else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };
    let Some(first) = parse_expression(ctx, first_node)? else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };

    let Some(second_node) = field!(recover, ctx, node, second_field) else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };
    let Some(second) = parse_expression(ctx, second_node)? else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };

    ctx.typechecking_context = saved_context;

    match node.kind() {
        "image_expr" => Ok(Some(Expression::Image(
            Metadata::new(),
            Moo::new(first),
            Moo::new(second),
        ))),
        "image_set_expr" => Ok(Some(Expression::ImageSet(
            Metadata::new(),
            Moo::new(first),
            Moo::new(second),
        ))),
        "pre_image_expr" => Ok(Some(Expression::PreImage(
            Metadata::new(),
            Moo::new(first),
            Moo::new(second),
        ))),
        "inverse_expr" => Ok(Some(Expression::Inverse(
            Metadata::new(),
            Moo::new(first),
            Moo::new(second),
        ))),
        "compose_expr" => Ok(Some(Expression::Compose(
            Metadata::new(),
            Moo::new(first),
            Moo::new(second),
        ))),
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Expected a function operator, got: {}", node.kind()),
                Some(node.range()),
            ));
            Ok(None)
        }
    }
}

/// Parses `restrict(f, dom)`. Unlike the other function operators, the second argument is
/// syntactically a domain rather than an expression. `Expression::Restrict` still stores it as
/// an `Expression` though (its `domain_of()` is called directly on that operand): a bare domain
/// name (e.g. `letting D be domain int(0,2)` used as `restrict(f, D)`) becomes a plain reference
/// to that declaration, matching the shape the via-conjure/JSON import path produces. An inline
/// domain literal (not just a name) is wrapped in a `DomainAnnotation` so `domain_of()` still
/// resolves correctly, though it will not print back out identically (no exhaustive/roundtrip
/// case in this codebase currently needs that form).
fn parse_restrict_expr(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    let saved_context = ctx.typechecking_context;
    ctx.typechecking_context = TypecheckingContext::Unknown;

    let Some(function_node) = field!(recover, ctx, node, "function") else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };
    let Some(function) = parse_expression(ctx, function_node)? else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };

    let Some(domain_node) = field!(recover, ctx, node, "domain") else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };
    let Some(domain_expr) = parse_restrict_domain_argument(ctx, domain_node)? else {
        ctx.typechecking_context = saved_context;
        return Ok(None);
    };

    ctx.typechecking_context = saved_context;

    Ok(Some(Expression::Restrict(
        Metadata::new(),
        Moo::new(function),
        Moo::new(domain_expr),
    )))
}

fn parse_restrict_domain_argument(
    ctx: &mut ParseContext,
    domain_node: Node,
) -> Result<Option<Expression>, FatalParseError> {
    // Unwrap the generic "domain"/"annotation_domain" wrapper node(s) to see what's underneath.
    let mut inner = domain_node;
    while matches!(inner.kind(), "domain" | "annotation_domain") {
        let Some(child) = inner.child(0) else {
            break;
        };
        inner = child;
    }

    if inner.kind() == "identifier" {
        // A name referring to a domain: keep it as a plain reference expression, so it prints
        // back out as just the name (e.g. `restrict(f,D)`).
        return parse_atom(ctx, &inner);
    }

    let Some(domain) = parse_domain(ctx, domain_node)? else {
        return Ok(None);
    };
    Ok(Some(Expression::DomainAnnotation(
        Metadata::new(),
        Moo::new(Expression::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool(true)),
        )),
        domain,
    )))
}

fn parse_index_or_slice(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    // add error and return early if we're in a set context, since indexing/slicing doesn't produce sets
    if ctx.typechecking_context == TypecheckingContext::Set {
        ctx.record_error(RecoverableParseError::new(
            format!(
                "Type error: {}\n\tExpected: set\n\tGot: index or slice",
                ctx.source_code[node.start_byte()..node.end_byte()].trim()
            ),
            Some(node.range()),
        ));
        return Ok(None);
    }

    // Save current context and temporarily set to Unknown for the collection
    let saved_context = ctx.typechecking_context;
    ctx.typechecking_context = TypecheckingContext::Unknown;
    let mut collection: Expression;
    match parse_atom(ctx, &field!(node, "collection"))? {
        Some(expr) => collection = expr,
        None => return Ok(None),
    }
    ctx.typechecking_context = saved_context;

    let indices_field = field!(node, "indices");
    let mut idx_nodes = named_children(&indices_field).collect::<VecDeque<_>>();

    // If LHS is a record, parse first index as a record field
    if let ReturnType::Record(ents) | ReturnType::Variant(ents) = collection.return_type() {
        let idx = idx_nodes
            .pop_front()
            .ok_or(FatalParseError::internal_error(
                "Expected at least one indexing expression".into(),
                Some(indices_field.range()),
            ))?;
        let name = Name::user(ctx.source_code[idx.start_byte()..idx.end_byte()].trim());
        let has_name = ents.iter().any(|e| e.name == name);
        if !has_name {
            return Err(FatalParseError::internal_error(
                format!("`{name}` is not a valid field name for `{collection}`"),
                Some(idx.range()),
            ));
        }
        collection = Expression::RecordField(Metadata::new(), Moo::new(collection), name);

        // If there are no more indices, return the record field directly
        if idx_nodes.is_empty() {
            return Ok(Some(collection));
        }
    }

    // Parse the rest of the indices as normal
    let mut indices = Vec::new();
    for idx_node in idx_nodes {
        indices.push(parse_index(ctx, &idx_node)?);
    }

    let has_null_idx = indices.iter().any(|idx| idx.is_none());
    // TODO: We could check whether the slice/index is safe here
    if has_null_idx {
        // It's a slice
        Ok(Some(Expression::UnsafeSlice(
            Metadata::new(),
            Moo::new(collection),
            indices,
        )))
    } else {
        // It's an index
        let idx_exprs: Vec<Expression> = indices.into_iter().map(|idx| idx.unwrap()).collect();
        Ok(Some(Expression::UnsafeIndex(
            Metadata::new(),
            Moo::new(collection),
            idx_exprs,
        )))
    }
}

fn parse_index(ctx: &mut ParseContext, node: &Node) -> Result<Option<Expression>, FatalParseError> {
    match node.kind() {
        "arithmetic_expr" | "atom" => {
            let saved_context = ctx.typechecking_context;
            ctx.typechecking_context = TypecheckingContext::Unknown;

            // TODO: add collection-aware index typechecking.
            // For tuple/matrix/set-like indexing, indices should be arithmetic.
            // For record field access, index atoms should resolve to valid field names.
            // This requires checking index expression together with the indexed collection type.

            let Some(expr) = parse_expression(ctx, *node)? else {
                return Ok(None);
            };

            ctx.typechecking_context = saved_context;
            Ok(Some(expr))
        }
        "null_index" => Ok(None),
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!("Expected an index, got: '{}'", node.kind()),
                Some(node.range()),
            ));
            Ok(None)
        }
    }
}

fn parse_variable(ctx: &mut ParseContext, node: &Node) -> Result<Option<Atom>, FatalParseError> {
    let raw_name = &ctx.source_code[node.start_byte()..node.end_byte()];

    let name = Name::user(raw_name.trim());
    if let Some(symbols) = &ctx.symbols {
        let lookup_result = {
            let symbols_read = symbols.read();
            symbols_read.lookup(&name)
        };

        if let Some(decl) = lookup_result {
            let symbol_kind = match &decl.kind().clone() as &DeclarationKind {
                DeclarationKind::Find(_) | DeclarationKind::FindAuxiliary(_) => SymbolKind::FindVar,
                DeclarationKind::Given(_) => SymbolKind::GivenVar,
                DeclarationKind::ValueLetting(_, _) => SymbolKind::LettingVar,
                DeclarationKind::TemporaryValueLetting(_) => SymbolKind::LettingVar,
                DeclarationKind::DomainLetting(_) => SymbolKind::LettingVar,
                DeclarationKind::Quantified(..) => SymbolKind::FindVar,
                DeclarationKind::QuantifiedExpr(..) => SymbolKind::FindVar,
                &_ => todo!(),
            };

            let hover = HoverInfo {
                description: format!("Variable: {name}"),
                doc_key: None,
                kind: Some(symbol_kind),
                ty: decl.domain().map(|d| d.to_string()),
                decl_span: ctx.lookup_decl_span(&name),
            };
            span_with_hover(node, ctx.source_code, ctx.source_map, hover);

            // Type check the variable against the expected context
            if let Some(error_msg) = typecheck_variable(&decl, ctx.typechecking_context, raw_name) {
                ctx.record_error(RecoverableParseError::new(error_msg, Some(node.range())));
                return Ok(None);
            }

            Ok(Some(Atom::Reference(conjure_cp_core::ast::Reference::new(
                decl,
            ))))
        } else {
            ctx.record_error(RecoverableParseError::new(
                format!("The identifier '{}' is not defined", raw_name),
                Some(node.range()),
            ));
            Ok(None)
        }
    } else {
        ctx.record_error(RecoverableParseError::new(
            format!("Symbol table missing when parsing variable '{raw_name}'"),
            Some(node.range()),
        ));
        Ok(None)
    }
}

/// Type check a variable declaration against the expected expression context.
/// Returns an error message if the variable type doesn't match the context.
fn typecheck_variable(
    decl: &DeclarationPtr,
    context: TypecheckingContext,
    raw_name: &str,
) -> Option<String> {
    // Only type check when context is known
    if context == TypecheckingContext::Unknown {
        return None;
    }

    // Get the variable's domain and resolve it
    let domain = decl.domain()?;
    let ground_domain = domain.resolve().ok()?;

    // Determine what type is expected
    let expected = match context {
        TypecheckingContext::Boolean => "bool",
        TypecheckingContext::Arithmetic => "int",
        TypecheckingContext::Set => "set",
        TypecheckingContext::SetOrMatrix => "set or matrix",
        TypecheckingContext::MSet => "mset",
        TypecheckingContext::Matrix => "matrix",
        TypecheckingContext::Tuple => "tuple",
        TypecheckingContext::Record => "record",
        TypecheckingContext::Partition => "partition",
        TypecheckingContext::Permutation => "permutation",
        TypecheckingContext::Sequence => "sequence",
        TypecheckingContext::Function => "function",
        TypecheckingContext::Relation => "relation",
        TypecheckingContext::Unknown => return None, // shouldn't reach here
    };

    // Determine what type we actually have
    let actual = match ground_domain.as_ref() {
        GroundDomain::Empty(_) => "empty",
        GroundDomain::Bool => "bool",
        GroundDomain::Int(_) => "int",
        GroundDomain::Tuple(_) => "tuple",
        GroundDomain::Record(_) => "record",
        GroundDomain::Variant(_) => "variant",
        GroundDomain::Matrix(_, _) => "matrix",
        GroundDomain::Sequence(_, _) => "sequence",
        GroundDomain::Set(_, _) => "set",
        GroundDomain::MSet(_, _) => "mset",
        GroundDomain::Function(_, _, _) => "function",
        GroundDomain::Relation(_, _) => "relation",
        GroundDomain::Partition(_, _) => "partition",
        GroundDomain::Permutation(_, _) => "permutation",
    };

    // If types match, no error
    if expected == actual
        || (context == TypecheckingContext::SetOrMatrix && matches!(actual, "set" | "matrix"))
    {
        return None;
    }

    // Otherwise, report the type mismatch
    Some(format!(
        "Type error: {}\n\tExpected: {}\n\tGot: {}",
        raw_name, expected, actual
    ))
}

fn parse_constant(ctx: &mut ParseContext, node: &Node) -> Result<Option<Literal>, FatalParseError> {
    let Some(inner) = named_child!(recover, ctx, node) else {
        return Ok(None);
    };
    let raw_value = &ctx.source_code[inner.start_byte()..inner.end_byte()];
    let lit = match inner.kind() {
        "integer" => {
            let Some(value) = parse_int(ctx, &inner) else {
                return Ok(None);
            };
            Literal::Int(value)
        }
        "TRUE" => {
            let hover = HoverInfo {
                description: format!("Boolean constant: {raw_value}"),
                doc_key: None,
                kind: None,
                ty: None,
                decl_span: None,
            };
            span_with_hover(&inner, ctx.source_code, ctx.source_map, hover);
            Literal::Bool(true)
        }
        "FALSE" => {
            let hover = HoverInfo {
                description: format!("Boolean constant: {raw_value}"),
                doc_key: None,
                kind: None,
                ty: None,
                decl_span: None,
            };
            span_with_hover(&inner, ctx.source_code, ctx.source_map, hover);
            Literal::Bool(false)
        }
        _ => {
            ctx.record_error(RecoverableParseError::new(
                format!(
                    "'{}' (kind: '{}') is not a valid constant",
                    raw_value,
                    inner.kind()
                ),
                Some(inner.range()),
            ));
            return Ok(None);
        }
    };

    // Type check the constant against the expected context
    if ctx.typechecking_context != TypecheckingContext::Unknown {
        let expected = match ctx.typechecking_context {
            TypecheckingContext::Boolean => "bool",
            TypecheckingContext::Arithmetic => "int",
            TypecheckingContext::Set => "set",
            TypecheckingContext::SetOrMatrix => "set or matrix",
            TypecheckingContext::MSet => "mset",
            TypecheckingContext::Matrix => "matrix",
            TypecheckingContext::Tuple => "tuple",
            TypecheckingContext::Record => "record",
            TypecheckingContext::Partition => "partition",
            TypecheckingContext::Permutation => "permutation",
            TypecheckingContext::Sequence => "sequence",
            TypecheckingContext::Function => "function",
            TypecheckingContext::Relation => "relation",
            TypecheckingContext::Unknown => "",
        };

        let actual = match &lit {
            Literal::Bool(_) => "bool",
            Literal::Int(_) => "int",
            Literal::AbstractLiteral(_) => return Ok(None), // Abstract literals aren't type-checked here
        };

        if expected != actual {
            ctx.record_error(RecoverableParseError::new(
                format!(
                    "Type error: {}\n\tExpected: {}\n\tGot: {}",
                    raw_value, expected, actual
                ),
                Some(node.range()),
            ));
            return Ok(None);
        }
    }
    Ok(Some(lit))
}

pub(crate) fn parse_int(ctx: &mut ParseContext, node: &Node) -> Option<i32> {
    let raw_value = &ctx.source_code[node.start_byte()..node.end_byte()];
    if let Ok(v) = raw_value.parse::<i32>() {
        Some(v)
    } else {
        ctx.record_error(RecoverableParseError::new(
            "Expected an integer here".to_string(),
            Some(node.range()),
        ));
        None
    }
}

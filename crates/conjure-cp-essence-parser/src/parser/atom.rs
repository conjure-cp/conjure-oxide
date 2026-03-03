use crate::diagnostics::diagnostics_api::SymbolKind;
use crate::diagnostics::source_map::{HoverInfo, span_with_hover};
use crate::errors::FatalParseError;
use crate::expression::{parse_binary_expression, parse_expression};
use crate::parser::ParseContext;
use crate::parser::abstract_literal::parse_abstract;
use crate::parser::comprehension::parse_comprehension;
use crate::util::named_children;
use crate::{field, named_child};
use conjure_cp_core::ast::{Atom, Expression, Literal, Metadata, Moo, Name};
use tree_sitter::Node;
use ustr::Ustr;

pub fn parse_atom(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    match node.kind() {
        "atom" | "sub_atom_expr" => parse_atom(ctx, &named_child!(node)),
        "metavar" => {
            let ident = field!(node, "identifier");
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
                return Err(FatalParseError::internal_error(
                    "fromSolution only allowed inside dominance relations".to_string(),
                    Some(node.range()),
                ));
            }

            let Some(inner) =
                parse_variable(ctx, &field!(node, "variable"))?
            else {
                return Ok(None);
            };

            Ok(Some(Expression::FromSolution(
                Metadata::new(),
                Moo::new(inner),
            )))
        }
        "constant" => {
            let lit = parse_constant(ctx, node)?;
            Ok(Some(Expression::Atomic(
                Metadata::new(),
                Atom::Literal(lit),
            )))
        }
        "matrix" | "record" | "tuple" | "set_literal" => {
            let Some(abs) = parse_abstract(ctx, node)? else {
                return Ok(None);
            };
            Ok(Some(Expression::AbstractLiteral(Metadata::new(), abs)))
        }
        "flatten" => parse_flatten(ctx, node),
        "index_or_slice" => parse_index_or_slice(ctx, node),
        // for now, assume is binary since powerset isn't implemented
        // TODO: add powerset support under "set_operation"
        "set_operation" => parse_binary_expression(ctx, node),
        "comprehension" => parse_comprehension(ctx, node),
        _ => Err(FatalParseError::internal_error(
            format!("Expected atom, got: {}", node.kind()),
            Some(node.range()),
        )),
    }
}

fn parse_flatten(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    let expr_node = field!(node, "expression");
    let Some(expr) = parse_atom(ctx, &expr_node)? else {
        return Ok(None);
    };

    if node.child_by_field_name("depth").is_some() {
        let depth_node = field!(node, "depth");
        let depth = parse_int(ctx, &depth_node)?;
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

fn parse_index_or_slice(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    let Some(collection) = parse_atom(ctx, &field!(node, "collection"))?
    else {
        return Ok(None);
    };
    let mut indices = Vec::new();
    for idx_node in named_children(&field!(node, "indices")) {
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

fn parse_index(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Expression>, FatalParseError> {
    match node.kind() {
        "arithmetic_expr" | "atom" => {
            let Some(expr) = parse_expression(ctx, *node)?
            else {
                return Ok(None);
            };
            Ok(Some(expr))
        }
        "null_index" => Ok(None),
        _ => Err(FatalParseError::internal_error(
            format!("Expected an index, got: '{}'", node.kind()),
            Some(node.range()),
        )),
    }
}

fn parse_variable(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Option<Atom>, FatalParseError> {
    let raw_name = &ctx.source_code[node.start_byte()..node.end_byte()];
    let name = Name::user(raw_name.trim());
    if let Some(symbols) = &ctx.symbols {
        if let Some(decl) = symbols.read().lookup(&name) {
            let hover = HoverInfo {
                description: format!("Variable: {name}"),
                kind: Some(SymbolKind::Decimal),
                ty: decl.domain().map(|d| d.to_string()),
                decl_span: None,
            };
            span_with_hover(node, ctx.source_code, ctx.source_map, hover);
            Ok(Some(Atom::Reference(conjure_cp_core::ast::Reference::new(
                decl,
            ))))
        } else {
            ctx.record_error(crate::errors::RecoverableParseError::new(
                format!("The identifier '{}' is not defined", raw_name),
                Some(node.range()),
            ));
            Ok(None)
        }
    } else {
        Err(FatalParseError::internal_error(
            format!("Symbol table missing when parsing variable '{raw_name}'"),
            Some(node.range()),
        ))
    }
}

fn parse_constant(
    ctx: &mut ParseContext,
    node: &Node,
) -> Result<Literal, FatalParseError> {
    let inner = named_child!(node);
    let raw_value = &ctx.source_code[inner.start_byte()..inner.end_byte()];
    match inner.kind() {
        "integer" => {
            let value = parse_int(ctx, &inner)?;
            Ok(Literal::Int(value))
        }
        "TRUE" => {
            let hover = HoverInfo {
                description: format!("Boolean constant: {raw_value}"),
                kind: None,
                ty: None,
                decl_span: None,
            };
            span_with_hover(&inner, ctx.source_code, ctx.source_map, hover);
            Ok(Literal::Bool(true))
        }
        "FALSE" => {
            let hover = HoverInfo {
                description: format!("Boolean constant: {raw_value}"),
                kind: None,
                ty: None,
                decl_span: None,
            };
            span_with_hover(&inner, ctx.source_code, ctx.source_map, hover);
            Ok(Literal::Bool(false))
        }
        _ => Err(FatalParseError::internal_error(
            format!(
                "'{}' (kind: '{}') is not a valid constant",
                raw_value,
                inner.kind()
            ),
            Some(inner.range()),
        )),
    }
}

pub(crate) fn parse_int(ctx: &ParseContext, node: &Node) -> Result<i32, FatalParseError> {
    let raw_value = &ctx.source_code[node.start_byte()..node.end_byte()];
    raw_value.parse::<i32>().map_err(|_e| {
        FatalParseError::internal_error("Expected an integer here".to_string(), Some(node.range()))
    })
}

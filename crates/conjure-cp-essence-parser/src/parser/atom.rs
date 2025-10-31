use crate::expression::{parse_binary_expression, parse_expression};
use crate::parser::abstract_literal::parse_abstract;
use crate::parser::comprehension::parse_comprehension;
use crate::util::named_children;
use crate::{EssenceParseError, field, named_child};
use conjure_cp_core::ast::{Atom, Expression, Literal, Metadata, Moo, Name, SymbolTable};
use std::cell::RefCell;
use std::rc::Rc;
use tree_sitter::Node;
use ustr::Ustr;

pub fn parse_atom(
    node: &Node,
    source_code: &str,
    root: &Node,
    symbols_ptr: Option<&Rc<RefCell<SymbolTable>>>,
) -> Result<Expression, EssenceParseError> {
    match node.kind() {
        "atom" => parse_atom(&named_child!(node), source_code, root, symbols_ptr),
        "metavar" => {
            let ident = field!(node, "identifier");
            let name_str = &source_code[ident.start_byte()..ident.end_byte()];
            Ok(Expression::Metavar(Metadata::new(), Ustr::from(name_str)))
        }
        "identifier" => parse_variable(node, source_code, symbols_ptr)
            .map(|var| Expression::Atomic(Metadata::new(), var)),
        "from_solution" => {
            if root.kind() != "dominance_relation" {
                return Err(EssenceParseError::syntax_error(
                    "fromSolution only allowed inside dominance relations".to_string(),
                    Some(node.range()),
                ));
            }

            let inner = parse_variable(&field!(node, "variable"), source_code, symbols_ptr)?;
            Ok(Expression::FromSolution(Metadata::new(), Moo::new(inner)))
        }
        "constant" => {
            let lit = parse_constant(node, source_code)?;
            Ok(Expression::Atomic(Metadata::new(), Atom::Literal(lit)))
        }
        "matrix" | "record" | "tuple" | "set_literal" => {
            parse_abstract(node, source_code, symbols_ptr)
                .map(|l| Expression::AbstractLiteral(Metadata::new(), l))
        }
        "tuple_matrix_record_index_or_slice" => {
            parse_index_or_slice(node, source_code, root, symbols_ptr)
        }
        // for now, assume is binary since powerset isn't implemented
        // TODO: add powerset support under "set_operation"
        "set_operation" => parse_binary_expression(node, source_code, root, symbols_ptr),
        "comprehension" => parse_comprehension(node, source_code, root, symbols_ptr),
        _ => Err(EssenceParseError::syntax_error(
            format!("Expected atom, got: {}", node.kind()),
            Some(node.range()),
        )),
    }
}

fn parse_index_or_slice(
    node: &Node,
    source_code: &str,
    root: &Node,
    symbols_ptr: Option<&Rc<RefCell<SymbolTable>>>,
) -> Result<Expression, EssenceParseError> {
    let collection = parse_atom(
        &field!(node, "tuple_or_matrix"),
        source_code,
        root,
        symbols_ptr,
    )?;
    let mut indices = Vec::new();
    for idx_node in named_children(&field!(node, "indices")) {
        indices.push(parse_index(&idx_node, source_code, symbols_ptr)?);
    }

    let has_null_idx = indices.iter().any(|idx| idx.is_none());
    // TODO: We could check whether the slice/index is safe here
    if has_null_idx {
        // It's a slice
        Ok(Expression::UnsafeSlice(
            Metadata::new(),
            Moo::new(collection),
            indices,
        ))
    } else {
        // It's an index
        let idx_exprs: Vec<Expression> = indices.into_iter().map(|idx| idx.unwrap()).collect();
        Ok(Expression::UnsafeIndex(
            Metadata::new(),
            Moo::new(collection),
            idx_exprs,
        ))
    }
}

fn parse_index(
    node: &Node,
    source_code: &str,
    symbols_ptr: Option<&Rc<RefCell<SymbolTable>>>,
) -> Result<Option<Expression>, EssenceParseError> {
    match node.kind() {
        "arithmetic_expr" => Ok(Some(parse_expression(
            *node,
            source_code,
            node,
            symbols_ptr,
        )?)),
        "null_index" => Ok(None),
        _ => Err(EssenceParseError::syntax_error(
            format!("Expected an index, got: '{}'", node.kind()),
            Some(node.range()),
        )),
    }
}

fn parse_variable(
    node: &Node,
    source_code: &str,
    symbols_ptr: Option<&Rc<RefCell<SymbolTable>>>,
) -> Result<Atom, EssenceParseError> {
    let raw_name = &source_code[node.start_byte()..node.end_byte()];
    let name = Name::user(raw_name.trim());
    if let Some(symbols) = symbols_ptr {
        if let Some(decl) = symbols.borrow().lookup(&name) {
            Ok(Atom::Reference(decl))
        } else {
            Err(EssenceParseError::syntax_error(
                format!("Undefined variable: '{}'", raw_name),
                Some(node.range()),
            ))
        }
    } else {
        Err(EssenceParseError::syntax_error(
            format!(
                "Found variable '{raw_name}'; Did you mean to pass a meta-variable '&{raw_name}'?\n\
            A symbol table is needed to resolve variable names, but none exists in this context."
            ),
            Some(node.range()),
        ))
    }
}

fn parse_constant(node: &Node, source_code: &str) -> Result<Literal, EssenceParseError> {
    let inner = named_child!(node);
    let raw_value = &source_code[inner.start_byte()..inner.end_byte()];
    match inner.kind() {
        "integer" => {
            let value = parse_int(&inner, source_code)?;
            Ok(Literal::Int(value))
        }
        "TRUE" => Ok(Literal::Bool(true)),
        "FALSE" => Ok(Literal::Bool(false)),
        _ => Err(EssenceParseError::syntax_error(
            format!(
                "'{}' (kind: '{}') is not a valid constant",
                raw_value,
                inner.kind()
            ),
            Some(inner.range()),
        )),
    }
}

fn parse_int(node: &Node, source_code: &str) -> Result<i32, EssenceParseError> {
    let raw_value = &source_code[node.start_byte()..node.end_byte()];
    raw_value.parse::<i32>().map_err(|_e| {
        if raw_value.is_empty() {
            EssenceParseError::syntax_error(
                "Expected an integer here".to_string(),
                Some(node.range()),
            )
        } else {
            EssenceParseError::syntax_error(
                format!("'{raw_value}' is not a valid integer"),
                Some(node.range()),
            )
        }
    })
}

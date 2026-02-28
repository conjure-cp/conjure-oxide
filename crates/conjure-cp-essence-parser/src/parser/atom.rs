use crate::errors::{FatalParseError, RecoverableParseError};
use crate::expression::{parse_binary_expression, parse_expression_with_context};
use crate::parser::abstract_literal::parse_abstract;
use crate::parser::comprehension::parse_comprehension;
use crate::util::named_children;
use crate::{field, named_child};
use conjure_cp_core::ast::{
    Atom, DeclarationPtr, Expression, GroundDomain, Literal, Metadata, Moo, Name, SymbolTablePtr,
};
use tree_sitter::Node;
use ustr::Ustr;

// Used to detect type mismatches during parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpressionContext {
    Boolean,
    Arithmetic,
    /// Context is unknown or flexible
    Unknown,
}

pub fn parse_atom(
    node: &Node,
    source_code: &str,
    root: &Node,
    symbols_ptr: Option<SymbolTablePtr>,
    errors: &mut Vec<RecoverableParseError>,
    context: ExpressionContext,
) -> Result<Option<Expression>, FatalParseError> {
    match node.kind() {
        "atom" | "sub_atom_expr" => {
            parse_atom(&named_child!(node), source_code, root, symbols_ptr, errors, context)
        }
        "metavar" => {
            let ident = field!(node, "identifier");
            let name_str = &source_code[ident.start_byte()..ident.end_byte()];
            Ok(Some(Expression::Metavar(
                Metadata::new(),
                Ustr::from(name_str),
            )))
        }
        "identifier" => {
            let Some(var) = parse_variable(node, source_code, symbols_ptr, errors, context)? else {
                return Ok(None);
            };
            Ok(Some(Expression::Atomic(Metadata::new(), var)))
        }
        "from_solution" => {
            if root.kind() != "dominance_relation" {
                return Err(FatalParseError::internal_error(
                    "fromSolution only allowed inside dominance relations".to_string(),
                    Some(node.range()),
                ));
            }

            let Some(inner) =
                parse_variable(&field!(node, "variable"), source_code, symbols_ptr, errors, context)?
            else {
                return Ok(None);
            };

            Ok(Some(Expression::FromSolution(
                Metadata::new(),
                Moo::new(inner),
            )))
        }
        "constant" => {
            let Some(lit) = parse_constant(node, source_code, errors, context)? else {
                return Ok(None);
            };
            
            Ok(Some(Expression::Atomic(
                Metadata::new(),
                Atom::Literal(lit),
            )))
        }
        "matrix" | "record" | "tuple" | "set_literal" => {
            let Some(abs) = parse_abstract(node, source_code, symbols_ptr, errors, context)? else {
                return Ok(None);
            };
            Ok(Some(Expression::AbstractLiteral(Metadata::new(), abs)))
        }
        "flatten" => parse_flatten(node, source_code, root, symbols_ptr, errors, context),
        "index_or_slice" => parse_index_or_slice(node, source_code, root, symbols_ptr, errors, context),
        // for now, assume is binary since powerset isn't implemented
        // TODO: add powerset support under "set_operation"
        "set_operation" => parse_binary_expression(node, source_code, root, symbols_ptr, errors, context),
        "comprehension" => parse_comprehension(node, source_code, root, symbols_ptr, errors),
        _ => Err(FatalParseError::internal_error(
            format!("Expected atom, got: {}", node.kind()),
            Some(node.range()),
        )),
    }
}

fn parse_flatten(
    node: &Node,
    source_code: &str,
    root: &Node,
    symbols_ptr: Option<SymbolTablePtr>,
    errors: &mut Vec<RecoverableParseError>,
    context: ExpressionContext,
) -> Result<Option<Expression>, FatalParseError> {
    let expr_node = field!(node, "expression");
    let Some(expr) = parse_atom(&expr_node, source_code, root, symbols_ptr, errors, context)? else {
        return Ok(None);
    };

    if node.child_by_field_name("depth").is_some() {
        let depth_node = field!(node, "depth");
        let depth = parse_int(&depth_node, source_code, errors)?;
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
    node: &Node,
    source_code: &str,
    root: &Node,
    symbols_ptr: Option<SymbolTablePtr>,
    errors: &mut Vec<RecoverableParseError>,
    _context: ExpressionContext,
) -> Result<Option<Expression>, FatalParseError> {
    let Some(collection) = parse_atom(
        &field!(node, "collection"),
        source_code,
        root,
        symbols_ptr.clone(),
        errors,
        ExpressionContext::Unknown, // don't enforce context on the collection itself
    )?
    else {
        return Ok(None);
    };
    let mut indices = Vec::new();
    for idx_node in named_children(&field!(node, "indices")) {
        indices.push(parse_index(
            &idx_node,
            source_code,
            symbols_ptr.clone(),
            errors,
        )?);
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
    node: &Node,
    source_code: &str,
    symbols_ptr: Option<SymbolTablePtr>,
    errors: &mut Vec<RecoverableParseError>,
) -> Result<Option<Expression>, FatalParseError> {
    match node.kind() {
        "arithmetic_expr" | "atom" => {
            let Some(expr) = parse_expression_with_context(*node, source_code, node, symbols_ptr, errors, ExpressionContext::Arithmetic)?
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

fn typecheck_variable(
    decl: &DeclarationPtr,
    var_name: &str,
    context: ExpressionContext,
) -> Option<String> {
    // Only type check when context is known
    if context == ExpressionContext::Unknown {
        return None;
    }

    // Get the variable's domain and resolve it
    let domain = decl.domain()?;
    let ground_domain = domain.resolve()?;

    let var_type = match ground_domain.as_ref() {
        GroundDomain::Int(_) => "Integer",
        GroundDomain::Bool => "Boolean",
        GroundDomain::Matrix(_, _) => "Matrix",
        GroundDomain::MSet(_, _) => "MSet",
        GroundDomain::Set(_, _) => "Set",
        GroundDomain::Tuple(_) => "Tuple",
        GroundDomain::Record(_) => "Record",
        _ => "The",
};

    match (context) {
        ExpressionContext::Boolean if var_type != "Boolean" => Some(format!(
            "Type error: {} variable '{}' cannot be used in boolean context",
            var_type, var_name
        )),
        ExpressionContext::Arithmetic if var_type != "Integer" => Some(format!(
            "Type error: {} variable '{}' cannot be used in arithmetic context",
            var_type, var_name
        )),
        _ => None,
    }
}

fn parse_variable(
    node: &Node,
    source_code: &str,
    symbols_ptr: Option<SymbolTablePtr>,
    errors: &mut Vec<RecoverableParseError>,
    context: ExpressionContext,
) -> Result<Option<Atom>, FatalParseError> {
    let raw_name = &source_code[node.start_byte()..node.end_byte()];
    let name = Name::user(raw_name.trim());
    
    if let Some(symbols) = symbols_ptr {
        if let Some(decl) = symbols.read().lookup(&name) {
            // Type check the variable against the expected context
            if let Some(error_msg) = typecheck_variable(&decl, raw_name, context) {
                errors.push(RecoverableParseError::new(error_msg, Some(node.range())));
                return Ok(None);
            }
            
            Ok(Some(Atom::Reference(conjure_cp_core::ast::Reference::new(
                decl,
            ))))
        } else {
            errors.push(RecoverableParseError::new(
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
    node: &Node,
    source_code: &str,
    errors: &mut Vec<RecoverableParseError>,
    context: ExpressionContext
) -> Result<Option<Literal>, FatalParseError> {
    let inner = named_child!(node);
    let raw_value = &source_code[inner.start_byte()..inner.end_byte()];
    let lit = match inner.kind() {
        "integer" => {
            let value = parse_int(&inner, source_code, errors)?;
            Literal::Int(value)
        }
        "TRUE" => Literal::Bool(true),
        "FALSE" => Literal::Bool(false),
        _ => return Err(FatalParseError::internal_error(
            format!(
                "'{}' (kind: '{}') is not a valid constant",
                raw_value,
                inner.kind()
            ),
            Some(inner.range()),
        )),
    };
    // Type check the constant against the expected context
    // lit with either be a boolean or an integer
    match (&lit, context) {
        (Literal::Bool(_), ExpressionContext::Arithmetic) => {
            errors.push(RecoverableParseError::new(
                format!("Type error: Boolean value '{}' used in arithmetic context", raw_value),
                Some(node.range()),
            ));
            return Ok(None);
        }
        (Literal::Int(_), ExpressionContext::Boolean) => {
            errors.push(RecoverableParseError::new(
                format!("Type error: Integer value '{}' used in boolean context", raw_value),
                Some(node.range()),
            ));
            return Ok(None);
        }
        _ => {}
    }
    Ok(Some(lit))
}

pub(crate) fn parse_int(
    node: &Node,
    source_code: &str,
    _errors: &mut Vec<RecoverableParseError>,
) -> Result<i32, FatalParseError> {
    let raw_value = &source_code[node.start_byte()..node.end_byte()];
    raw_value.parse::<i32>().map_err(|_e| {
        FatalParseError::internal_error("Expected an integer here".to_string(), Some(node.range()))
    })
}

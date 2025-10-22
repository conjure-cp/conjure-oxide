//! Functions for converting from a Conjure Model to assertions in Z3.
//!
//! Conversions are mostly 1-to-1 since any rewriting was done previously using rules.
//! We transform the AST bottom-up, each recursion returning a dynamic value, and the one above
//! interpreting it as the type it expects or failing.
//!
//! "Dynamic" or "AST" is used to describe generic Z3 AST values. "Expression" means
//! a Conjure Oxide [`Expression`] type.

use z3::Solver;
use z3::Symbol;
use z3::ast::*;

use super::store::Store;

use crate::Model;
use crate::ast::*;
use crate::solver::SolverError;

/// Adds variables from the symbol table to the store to be referenced when adding constraints.
pub fn load_store(store: &mut Store, symbols: &SymbolTable) -> Result<(), SolverError> {
    for (name, decl) in symbols.clone().into_iter_local() {
        let Some(var) = decl.as_var() else {
            continue;
        };
        let ast = var_to_ast(&name, &var)?;
        store.insert(name, ast);
    }
    Ok(())
}

/// Converts top-level expressions into assertions on the model.
pub fn load_assertions(
    store: &Store,
    model: &[Expression],
    solver: &mut Solver,
) -> Result<(), SolverError> {
    for expr in model.iter() {
        let bool = expr_to_ast(store, expr)?
            .as_bool()
            .ok_or(SolverError::ModelInvalid(format!(
                "top-level expression must be boolean type: {expr}",
            )))?;
        solver.assert(bool);
    }
    Ok(())
}

/// Creates an AST of the relevant type for a given decision variable.
fn var_to_ast(name: &Name, decl: &DecisionVariable) -> Result<Dynamic, SolverError> {
    let sym = name_to_symbol(name)?;
    match decl.domain {
        Domain::Bool => Ok(Dynamic::from_ast(&Bool::new_const(sym))),
        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "domain kind for '{name}' not implemented: {}",
            decl.domain
        ))),
    }
}

fn name_to_symbol(name: &Name) -> Result<Symbol, SolverError> {
    match name {
        Name::User(ustr) => Ok(Symbol::String((*ustr).into())),
        Name::Machine(num) => Ok(Symbol::Int(*num as u32)),
        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "variable '{name}' is part of a representation"
        ))),
    }
}

/// Converts a Conjure Oxide Expression to a dynamic AST node for Z3.
fn expr_to_ast(store: &Store, expr: &Expression) -> Result<Dynamic, SolverError> {
    match expr {
        Expression::Atomic(_, atom) => atom_to_ast(store, atom),

        // Since equality is part of the core SMT theory, any two dynamic ASTs
        // of the same type can be compared with it.
        // Since we don't need to convert to a specific type we use a conversion function
        // which just clones the AST input.
        Expression::Neq(_, a, b) => binary_op(store, a, b, ast_id, ast_id, |a, b| a.ne(b)),
        Expression::Eq(_, a, b) => binary_op(store, a, b, ast_id, ast_id, |a, b| a.eq(b)),

        // === Boolean Expressions ===
        Expression::Not(_, a) => unary_op(store, a, Dynamic::as_bool, |a| a.not()),
        Expression::Imply(_, a, b) => {
            binary_op(store, a, b, Dynamic::as_bool, Dynamic::as_bool, |a, b| {
                a.implies(b)
            })
        }
        Expression::Iff(_, a, b) => {
            binary_op(store, a, b, Dynamic::as_bool, Dynamic::as_bool, |a, b| {
                a.iff(b)
            })
        }
        Expression::Or(_, a) => {
            let exprs = list_to_vec(a)?;
            vec_op(store, exprs.as_slice(), Dynamic::as_bool, |asts| {
                Bool::or(asts)
            })
        }
        Expression::And(_, a) => {
            let exprs = list_to_vec(a)?;
            vec_op(store, exprs.as_slice(), Dynamic::as_bool, |asts| {
                Bool::and(asts)
            })
        }

        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "expression type not yet implemented: {expr}"
        ))),
    }
}

/// Converts an atom (literal or reference) into an AST node.
fn atom_to_ast(store: &Store, atom: &Atom) -> Result<Dynamic, SolverError> {
    match atom {
        Atom::Reference(decl) => store
            .get(&decl.name())
            .ok_or(SolverError::ModelInvalid(format!(
                "variable '{}' does not exist",
                decl.name()
            )))
            .cloned(),
        Atom::Literal(lit) => literal_to_ast(lit),
    }
}

/// Converts a CO literal (expression containing no variables) into an AST node.
fn literal_to_ast(lit: &Literal) -> Result<Dynamic, SolverError> {
    match lit {
        Literal::Bool(b) => Ok(Bool::from_bool(*b).into()),
        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "literal type not implemented: {lit}"
        ))),
    }
}

/// Turns a list expression into a vector of its expressions.
/// A list is a matrix with unbounded domain `[n..)`.
fn list_to_vec(expr: &Expression) -> Result<Vec<Expression>, SolverError> {
    expr.clone()
        .unwrap_list()
        .ok_or(SolverError::ModelFeatureNotImplemented(format!(
            "inner expression must be a list: {expr}"
        )))
}

/// Applies a conversion to a Dynamic, usually to some other AST type.
/// Transforms the resulting Option into a Result possibly containing an error.
fn conv_ast<From>(
    ast: Dynamic,
    conv: impl Fn(&Dynamic) -> Option<From>,
) -> Result<From, SolverError> {
    conv(&ast).ok_or(SolverError::ModelInvalid(format!(
        "conversion failed on: {ast}"
    )))
}

fn ast_id(ast: &Dynamic) -> Option<Dynamic> {
    Some(ast.clone())
}

/// Interprets an expression as an AST, converts it using the given conversion,
/// and passes the result to the given unary operator closure.
///
/// Since [`expr_to_ast`] returns a dynamic AST value, conversions are a convenient
/// way to make the input type to the operators correct. For example, in the case of
/// the `implies` operator the conversion lets us convert the returned dynamic values
/// to booleans.
fn unary_op<FromA, Out>(
    store: &Store,
    a: &Expression,
    conv_a: impl Fn(&Dynamic) -> Option<FromA>,
    op: impl Fn(FromA) -> Out,
) -> Result<Dynamic, SolverError>
where
    Out: Into<Dynamic>,
{
    let a_ast = conv_ast(expr_to_ast(store, a)?, &conv_a)?;
    Ok((op)(a_ast).into())
}

/// Interprets two expressions as ASTs, converts them using the given conversion
/// closures, and passes the results to the given unary operator closure.
///
/// And example of this is logical implication, where the conversion for both operands
/// is to `Bool` and the operator returns `Bool::implies(a, b)`.
fn binary_op<FromA, FromB, Out>(
    store: &Store,
    a: &Expression,
    b: &Expression,
    conv_a: impl Fn(&Dynamic) -> Option<FromA>,
    conv_b: impl Fn(&Dynamic) -> Option<FromB>,
    op: impl Fn(FromA, FromB) -> Out,
) -> Result<Dynamic, SolverError>
where
    Out: Into<Dynamic>,
{
    let a_ast = conv_ast(expr_to_ast(store, a)?, conv_a)?;
    let b_ast = conv_ast(expr_to_ast(store, b)?, conv_b)?;
    Ok((op)(a_ast, b_ast).into())
}

/// Transforms a slice of expressions into ASTs, converts them using the same conversion
/// closure, and passes the resulting slice to the operator closure.
///
/// An example of this is a conjunction, where the conversion is to `Bool` and the operator
/// returns `Bool::and(slice)`.
fn vec_op<From, Out>(
    store: &Store,
    exprs: &[Expression],
    conv: impl Fn(&Dynamic) -> Option<From>,
    op: impl Fn(&[From]) -> Out,
) -> Result<Dynamic, SolverError>
where
    Out: Into<Dynamic>,
{
    // Result implements FromIter, collecting into either the full collection or an error
    let asts: Result<Vec<_>, SolverError> = exprs
        .iter()
        .map(|e| expr_to_ast(store, e).and_then(|ast| conv_ast(ast, &conv)))
        .collect();
    let asts = asts?;

    Ok((op)(asts.as_slice()).into())
}

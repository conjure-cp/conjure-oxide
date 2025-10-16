//! Functions for converting from a Conjure Model to assertions in Z3.
//!
//! Conversions are mostly 1-to-1 since any rewriting was done previously using rules.
//! We recursively transform the AST bottom-up, returning the requested AST type (e.g. Bool, Int)
//! or an error.
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
        let bool: Bool = expr_to_ast(store, expr)?;
        solver.assert(bool);
    }
    Ok(())
}

/// Creates an AST of the relevant type for a given decision variable.
fn var_to_ast(name: &Name, decl: &DecisionVariable) -> Result<Dynamic, SolverError> {
    let sym = name_to_symbol(name)?;
    match decl.domain {
        Domain::Bool => Ok(Bool::new_const(sym).into()),
        Domain::Int(_) => Ok(Int::new_const(sym).into()),
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

/// Converts a Conjure Oxide Expression to an AST node for Z3.
///
/// The generic type parameter lets us cast the result to a specific return type.
fn expr_to_ast<Out>(store: &Store, expr: &Expression) -> Result<Out, SolverError>
where
    Out: TryFrom<Dynamic, Error: std::fmt::Display>,
{
    let ast = match expr {
        Expression::Atomic(_, atom) => atom_to_ast(store, atom),

        // Equality is part of the SMT core theory (anything can be compared)
        // TODO: use safe eq
        Expression::Neq(_, a, b) => binary_op(store, a, b, |a: Dynamic, b: Dynamic| a.ne(b)),
        Expression::Eq(_, a, b) => binary_op(store, a, b, |a: Dynamic, b: Dynamic| a.eq(b)),

        // The below operations are grouped by return type and then by arity

        // === Boolean Expressions ===
        Expression::Not(_, a) => unary_op(store, a, |a: Bool| a.not()),

        Expression::Imply(_, a, b) => binary_op(store, a, b, |a: Bool, b: Bool| a.implies(b)),
        Expression::Iff(_, a, b) => binary_op(store, a, b, |a: Bool, b: Bool| a.iff(b)),
        Expression::Gt(_, a, b) => binary_op(store, a, b, |a: Int, b: Int| a.gt(b)),
        Expression::Lt(_, a, b) => binary_op(store, a, b, |a: Int, b: Int| a.lt(b)),
        Expression::Geq(_, a, b) => binary_op(store, a, b, |a: Int, b: Int| a.ge(b)),
        Expression::Leq(_, a, b) => binary_op(store, a, b, |a: Int, b: Int| a.le(b)),

        Expression::Or(_, a) => {
            let exprs = list_to_vec(a)?;
            many_op(store, exprs.as_slice(), |asts: &[Bool]| Bool::or(asts))
        }
        Expression::And(_, a) => {
            let exprs = list_to_vec(a)?;
            many_op(store, exprs.as_slice(), |asts: &[Bool]| Bool::and(asts))
        }

        // === Integer Expressions ===
        Expression::Neg(_, a) => unary_op(store, a, |a: Int| a.unary_minus()),
        Expression::ToInt(_, a) => {
            unary_op(store, a, |a: Bool| a.ite(&Int::from(1), &Int::from(0)))
        }
        Expression::Abs(_, a) => unary_op(store, a, |a: Int| {
            a.lt(Int::from(0)).ite(&a.unary_minus(), &a)
        }),

        Expression::SafeDiv(_, a, b) => binary_op(store, a, b, |a: Int, b: Int| a.div(b)),
        Expression::SafeMod(_, a, b) => binary_op(store, a, b, |a: Int, b: Int| a.modulo(b)),
        Expression::SafePow(_, a, b) => binary_op(store, a, b, |a: Int, b: Int| a.power(b)),

        Expression::Product(_, a) => {
            let exprs = list_to_vec(a)?;
            many_op(store, exprs.as_slice(), |asts: &[Int]| Int::mul(asts))
        }
        Expression::Sum(_, a) => {
            let exprs = list_to_vec(a)?;
            many_op(store, exprs.as_slice(), |asts: &[Int]| Int::add(asts))
        }

        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "expression type not implemented: {expr}"
        ))),
    }?;

    ast.try_into().map_err(|err| {
        SolverError::ModelInvalid(format!(
            "expression has incorrect type for conversion: {err}"
        ))
    })
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
        Literal::Int(n) => Ok(Int::from(*n).into()),
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

/// Interprets an expression as an AST and returns the result of the given operation over it.
fn unary_op<A, Out>(
    store: &Store,
    a: &Expression,
    op: impl FnOnce(A) -> Out,
) -> Result<Dynamic, SolverError>
where
    A: TryFrom<Dynamic, Error: std::fmt::Display>,
    Out: Into<Dynamic>,
{
    let a_ast: A = expr_to_ast(store, a)?;
    Ok((op)(a_ast).into())
}

/// Interprets two expressions as ASTs and returns the result of the given operation over them.
fn binary_op<A, B, Out>(
    store: &Store,
    a: &Expression,
    b: &Expression,
    op: impl FnOnce(A, B) -> Out,
) -> Result<Dynamic, SolverError>
where
    A: TryFrom<Dynamic, Error: std::fmt::Display>,
    B: TryFrom<Dynamic, Error: std::fmt::Display>,
    Out: Into<Dynamic>,
{
    let a_ast: A = expr_to_ast(store, a)?;
    let b_ast: B = expr_to_ast(store, b)?;
    Ok((op)(a_ast, b_ast).into())
}

/// Transforms a slice of expressions into ASTs and returns the result of the given operation over it.
fn many_op<A, Out>(
    store: &Store,
    exprs: &[Expression],
    op: impl FnOnce(&[A]) -> Out,
) -> Result<Dynamic, SolverError>
where
    A: TryFrom<Dynamic, Error: std::fmt::Display>,
    Out: Into<Dynamic>,
{
    // Result implements FromIter, collecting into either the full collection or an error
    let asts_res: Result<Vec<_>, SolverError> =
        exprs.iter().map(|e| expr_to_ast(store, e)).collect();
    let asts = asts_res?;

    Ok((op)(asts.as_slice()).into())
}

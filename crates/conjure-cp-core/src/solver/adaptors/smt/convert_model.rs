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

/// Converts the given variables and constraints to assertions by mutating the given model.
///
/// SMT does not use bounded domains the same way Conjure Oxide does; for example integers
/// domains are unbounded. For this reason, additional assertions are made to keep these
/// variables within their domains.
pub fn load_model_impl(
    store: &mut Store,
    solver: &mut Solver,
    symbols: &SymbolTable,
    model: &[Expression],
) -> Result<(), SolverError> {
    for (name, decl) in symbols.clone().into_iter_local() {
        let Some(var) = decl.as_var() else {
            continue;
        };
        let (ast, restriction) = var_to_ast(&name, &var)?;
        store.insert(name, ast);
        solver.assert(restriction);
    }
    for expr in model.iter() {
        let bool: Bool = expr_to_ast(store, expr)?;
        solver.assert(bool);
    }
    Ok(())
}

/// Returns the AST representation of the variable as well as a boolean assertion to restrict
/// it to the input variable's domain (e.g. integers have unbounded domains in SMT).
fn var_to_ast(name: &Name, var: &DecisionVariable) -> Result<(Dynamic, Bool), SolverError> {
    let sym = name_to_symbol(name)?;
    match &var.domain {
        // Booleans of course have the same domain in SMT, so no restriction required
        Domain::Bool => Ok((Bool::new_const(sym).into(), Bool::from_bool(true))),

        // Return a disjunction of the restrictions each range of the domain enforces
        // I.e. `x: int(1, 3..5)` -> `or([x = 1, x >= 3 /\ x <= 5])`
        Domain::Int(ranges) => {
            let sym_const = Int::new_const(sym);
            let restrictions_res: Result<Vec<_>, SolverError> = ranges
                .iter()
                .map(|range| int_range_to_bool(&sym_const, range))
                .collect();
            let restrictions = restrictions_res?;
            Ok((sym_const.into(), Bool::or(restrictions.as_slice())))
        }

        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "domain kind for '{name}' not implemented: {}",
            var.domain
        ))),
    }
}

fn int_range_to_bool(sym_const: &Int, range: &Range<i32>) -> Result<Bool, SolverError> {
    match range {
        Range::Single(n) => Ok(sym_const.eq(Int::from(*n))),
        Range::UnboundedL(r) => Ok(sym_const.le(Int::from(*r))),
        Range::UnboundedR(l) => Ok(sym_const.ge(Int::from(*l))),
        Range::Bounded(l, r) => {
            Ok({ Bool::and(&[sym_const.ge(Int::from(*l)), sym_const.le(Int::from(*r))]) })
        }
        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "range type not implemented: {range}"
        ))),
    }
}

fn name_to_symbol(name: &Name) -> Result<Symbol, SolverError> {
    match name {
        Name::User(ustr) => Ok(Symbol::String((*ustr).into())),
        Name::Machine(num) => Ok(Symbol::Int(*num as u32)),
        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "variable '{name}' name is unsupported"
        ))),
    }
}

/// Converts a Conjure Oxide Expression to an AST node for Z3.
/// The generic type parameter lets us cast the result to a specific return type.
fn expr_to_ast<Out>(store: &Store, expr: &Expression) -> Result<Out, SolverError>
where
    Out: TryFrom<Dynamic, Error: std::fmt::Display>,
{
    let ast = match expr {
        Expression::Atomic(_, atom) => atom_to_ast(store, atom),

        // Equality is part of the SMT core theory (anything can be compared)
        // We do some extra work to return a clean error if the types are different
        Expression::Eq(_, a, b) => checked_binary_op(store, a, b, |a: Dynamic, b: Dynamic| {
            a.safe_eq(b)
                .map_err(|err| SolverError::ModelInvalid(err.to_string()))
        })?,
        Expression::Neq(_, a, b) => checked_binary_op(store, a, b, |a: Dynamic, b: Dynamic| {
            a.safe_eq(b)
                .map(|eq| eq.not())
                .map_err(|err| SolverError::ModelInvalid(err.to_string()))
        })?,

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
        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "atom sort not implemented: {atom}"
        ))),
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

/// Interprets two expressions as ASTs and returns the result of the given operation over
/// them (which may be an error).
fn checked_binary_op<A, B, Error, Out>(
    store: &Store,
    a: &Expression,
    b: &Expression,
    op: impl FnOnce(A, B) -> Result<Out, Error>,
) -> Result<Result<Dynamic, Error>, SolverError>
where
    A: TryFrom<Dynamic, Error: std::fmt::Display>,
    B: TryFrom<Dynamic, Error: std::fmt::Display>,
    Out: Into<Dynamic>,
{
    let a_ast: A = expr_to_ast(store, a)?;
    let b_ast: B = expr_to_ast(store, b)?;
    Ok((op)(a_ast, b_ast).map(Into::into))
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

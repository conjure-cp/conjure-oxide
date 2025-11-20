//! Functions for converting from a Conjure Model to assertions in Z3.
//!
//! Conversions are mostly 1-to-1 since any rewriting was done previously using rules.
//! We recursively transform the AST bottom-up, returning the requested AST type (e.g. Bool, Int)
//! or an error.
//!
//! "Dynamic" or "AST" is used to describe generic Z3 AST values. "Expression" means
//! a Conjure Oxide [`Expression`] type.

use std::collections::HashSet;

use z3::ast::*;
use z3::{Solver, Sort, Symbol};

use super::helpers::*;
use super::store::SymbolStore;
use super::{IntTheory, TheoryConfig};

use crate::Model;
use crate::ast::*;
use crate::solver::{SolverError, SolverResult};

/// Converts the given variables and constraints to assertions by mutating the given model.
///
/// SMT does not use bounded domains the same way Conjure Oxide does; for example integers
/// domains are unbounded. For this reason, additional assertions are made to keep these
/// variables within their domains.
pub fn load_model_impl(
    store: &mut SymbolStore,
    solver: &mut Solver,
    theory_config: &TheoryConfig,
    symbols: &SymbolTable,
    model: &[Expression],
) -> SolverResult<()> {
    for (name, decl) in symbols.clone().into_iter_local() {
        let Some(var) = decl.as_var() else {
            /// Ignore lettings, etc
            continue;
        };
        if !symbols
            .representations_for(&name)
            .is_none_or(|reps| reps.is_empty())
        {
            /// This variable has representations; ignore it
            continue;
        }
        let (sym, ast, restriction) = var_to_ast(&name, &var, theory_config)?;
        store.insert(name, (decl.domain().unwrap(), ast, sym));
        solver.assert(restriction);
    }
    for expr in model.iter() {
        let bool: Bool = expr_to_ast(store, expr, theory_config)?;
        solver.assert(bool);
    }
    Ok(())
}

/// Returns the AST representation of the variable as well as a boolean assertion which restricts
/// it to the input variable's domain since most Z3 sorts are unbounded.
fn var_to_ast(
    name: &Name,
    var: &DecisionVariable,
    theories: &TheoryConfig,
) -> SolverResult<(Symbol, Dynamic, Bool)> {
    let sym = name_to_symbol(name)?;
    let (sort, restrict_fn) = domain_to_sort(&var.domain, theories)?;
    let new_const = Dynamic::new_const(sym.clone(), &sort);

    let restriction = (restrict_fn)(&new_const);
    Ok((sym, new_const, restriction))
}

/// Converts a Conjure Oxide Expression to an AST node for Z3.
/// The generic type parameter lets us cast the result to a specific return type.
fn expr_to_ast<Out>(store: &SymbolStore, expr: &Expression, thr: &TheoryConfig) -> SolverResult<Out>
where
    Out: TryFrom<Dynamic, Error: std::fmt::Display>,
{
    use IntTheory::{Bv, Lia};

    // Some translations are only allowed in certain theories.
    // E.g. if using LIA for ints then Expression::Sum is allowed, where in BV it must be PairwiseSum.
    let ast = match (thr.ints, expr) {
        (_, Expression::Atomic(_, atom)) => atom_to_ast(thr, store, atom),

        // Equality is part of the SMT core theory (anything can be compared)
        (_, Expression::Eq(_, a, b)) => {
            binary_op(thr, store, a, b, |a: Dynamic, b: Dynamic| a.eq(b))
        }

        (_, Expression::Neq(_, a, b)) => {
            binary_op(thr, store, a, b, |a: Dynamic, b: Dynamic| a.ne(b))
        }

        // === Boolean Expressions ===
        (_, Expression::Not(_, a)) => unary_op(thr, store, a, |a: Bool| a.not()),

        (_, Expression::Imply(_, a, b)) => {
            binary_op(thr, store, a, b, |a: Bool, b: Bool| a.implies(b))
        }
        (_, Expression::Iff(_, a, b)) => binary_op(thr, store, a, b, |a: Bool, b: Bool| a.iff(b)),
        (_, Expression::Or(_, a)) => list_op(thr, store, a, |asts: &[Bool]| Bool::or(asts)),
        (_, Expression::And(_, a)) => list_op(thr, store, a, |asts: &[Bool]| Bool::and(asts)),

        // === Expressions involving integers: Linear Integer Arithmetic theory ===
        (Lia, Expression::Neg(_, a)) => unary_op(thr, store, a, |a: Int| a.unary_minus()),
        (Lia, Expression::ToInt(_, a)) => {
            unary_op(thr, store, a, |a: Bool| a.ite(&Int::from(1), &Int::from(0)))
        }
        (Lia, Expression::Abs(_, a)) => unary_op(thr, store, a, |a: Int| {
            a.lt(Int::from(0)).ite(&a.unary_minus(), &a)
        }),
        (Lia, Expression::SafeDiv(_, a, b)) => {
            binary_op(thr, store, a, b, |a: Int, b: Int| a.div(b))
        }
        (Lia, Expression::SafeMod(_, a, b)) => {
            binary_op(thr, store, a, b, |a: Int, b: Int| a.modulo(b))
        }
        (Lia, Expression::SafePow(_, a, b)) => {
            binary_op(thr, store, a, b, |a: Int, b: Int| a.power(b))
        }
        (Lia, Expression::Gt(_, a, b)) => binary_op(thr, store, a, b, |a: Int, b: Int| a.gt(b)),
        (Lia, Expression::Lt(_, a, b)) => binary_op(thr, store, a, b, |a: Int, b: Int| a.lt(b)),
        (Lia, Expression::Geq(_, a, b)) => binary_op(thr, store, a, b, |a: Int, b: Int| a.ge(b)),
        (Lia, Expression::Leq(_, a, b)) => binary_op(thr, store, a, b, |a: Int, b: Int| a.le(b)),
        (Lia, Expression::Product(_, a)) => list_op(thr, store, a, |asts: &[Int]| Int::mul(asts)),
        (Lia, Expression::Sum(_, a)) => list_op(thr, store, a, |asts: &[Int]| Int::add(asts)),

        // === Expressions involving integers: Fixed-Size Bit Vector theory ===
        // TODO: check for overflow on relevant operations?
        (Bv, Expression::Neg(_, a)) => unary_op(thr, store, a, |a: BV| a.bvneg()),
        (Bv, Expression::ToInt(_, a)) => unary_op(thr, store, a, |a: Bool| {
            a.ite(&BV::from_i64(1, BV_SIZE), &BV::from_i64(0, BV_SIZE))
        }),
        (Bv, Expression::Abs(_, a)) => unary_op(thr, store, a, |a: BV| {
            a.bvslt(BV::from_i64(0, BV_SIZE)).ite(&a.bvneg(), &a)
        }),
        (Bv, Expression::SafeDiv(_, a, b)) => {
            binary_op(thr, store, a, b, |a: BV, b: BV| a.bvsdiv(b))
        }
        (Bv, Expression::SafeMod(_, a, b)) => {
            binary_op(thr, store, a, b, |a: BV, b: BV| a.bvsrem(b))
        }
        (Bv, Expression::SafePow(_, a, b)) => todo!(),

        (Bv, Expression::PairwiseSum(_, a, b)) => {
            binary_op(thr, store, a, b, |a: BV, b: BV| a.bvadd(b))
        }
        (Bv, Expression::PairwiseProduct(_, a, b)) => {
            binary_op(thr, store, a, b, |a: BV, b: BV| a.bvmul(b))
        }
        (Bv, Expression::Gt(_, a, b)) => binary_op(thr, store, a, b, |a: BV, b: BV| a.bvsgt(b)),
        (Bv, Expression::Lt(_, a, b)) => binary_op(thr, store, a, b, |a: BV, b: BV| a.bvslt(b)),
        (Bv, Expression::Geq(_, a, b)) => binary_op(thr, store, a, b, |a: BV, b: BV| a.bvsge(b)),
        (Bv, Expression::Leq(_, a, b)) => binary_op(thr, store, a, b, |a: BV, b: BV| a.bvsle(b)),

        // === Expressions involving matrices ===
        (_, Expression::SafeIndex(_, m, idxs)) => {
            let arr: Dynamic = expr_to_ast(store, m, thr)?;
            slice_op(thr, store, idxs, move |idxs: &[Dynamic]| {
                idxs.iter()
                    .fold(arr, |cur_arr, idx| cur_arr.as_array().unwrap().select(idx))
            })
        }
        (_, Expression::AllDiff(_, a)) => {
            list_op(thr, store, a, |asts: &[Dynamic]| Dynamic::distinct(asts))
        }

        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "expression type not implemented for theories `{thr:?}`: {expr}"
        ))),
    }?;

    ast.try_into().map_err(|err| {
        SolverError::ModelInvalid(format!(
            "expression has incorrect type for conversion: {err}"
        ))
    })
}

/// Interprets an expression as an AST and returns the result of the given operation over it.
fn unary_op<A, Out>(
    theories: &TheoryConfig,
    store: &SymbolStore,
    a: &Expression,
    op: impl FnOnce(A) -> Out,
) -> SolverResult<Dynamic>
where
    A: TryFrom<Dynamic, Error: std::fmt::Display>,
    Out: Into<Dynamic>,
{
    let a_ast: A = expr_to_ast(store, a, theories)?;
    Ok((op)(a_ast).into())
}

/// Interprets two expressions as ASTs and returns the result of the given operation over them.
fn binary_op<A, B, Out>(
    theories: &TheoryConfig,
    store: &SymbolStore,
    a: &Expression,
    b: &Expression,
    op: impl FnOnce(A, B) -> Out,
) -> SolverResult<Dynamic>
where
    A: TryFrom<Dynamic, Error: std::fmt::Display>,
    B: TryFrom<Dynamic, Error: std::fmt::Display>,
    Out: Into<Dynamic>,
{
    let a_ast: A = expr_to_ast(store, a, theories)?;
    let b_ast: B = expr_to_ast(store, b, theories)?;
    Ok((op)(a_ast, b_ast).into())
}

/// Transforms a list expression into separate ASTs and returns the result of the given operation over them.
fn list_op<A, Out>(
    theories: &TheoryConfig,
    store: &SymbolStore,
    expr: &Expression,
    op: impl FnOnce(&[A]) -> Out,
) -> SolverResult<Dynamic>
where
    A: TryFrom<Dynamic, Error: std::fmt::Display>,
    Out: Into<Dynamic>,
{
    let exprs = expr
        .clone()
        .unwrap_list()
        .ok_or(SolverError::ModelFeatureNotImplemented(format!(
            "inner expression must be a list: {expr}"
        )))?;

    slice_op(theories, store, &exprs, op)
}

/// Transforms a slice of expressions into ASTs and returns the result of the given operation over it.
fn slice_op<A, Out>(
    theories: &TheoryConfig,
    store: &SymbolStore,
    exprs: &[Expression],
    op: impl FnOnce(&[A]) -> Out,
) -> SolverResult<Dynamic>
where
    A: TryFrom<Dynamic, Error: std::fmt::Display>,
    Out: Into<Dynamic>,
{
    // Result implements FromIter, collecting into either the full collection or an error
    let asts_res: SolverResult<Vec<_>> = exprs
        .iter()
        .map(|e| expr_to_ast(store, e, theories))
        .collect();
    let asts = asts_res?;

    Ok((op)(asts.as_slice()).into())
}

//! Functions for converting from a Conjure Model to assertions in Z3.
//!
//! Conversions are mostly 1-to-1 since any rewriting was done previously using rules.
//! We recursively transform the AST bottom-up, returning the requested AST type (e.g. Bool, Int)
//! or an error.
//!
//! "Dynamic" or "AST" is used to describe generic Z3 AST values. "Expression" means
//! a Conjure Oxide [`Expression`] type.

use z3::ast::*;
use z3::{Solver, Symbol};

use super::store::Store;
use super::{IntTheory, TheoryConfig};

use crate::Model;
use crate::ast::*;
use crate::solver::SolverError;

/// Use 32-bit 2's complement signed bit-vectors
const BV_SIZE: u32 = 32;

/// Converts the given variables and constraints to assertions by mutating the given model.
///
/// SMT does not use bounded domains the same way Conjure Oxide does; for example integers
/// domains are unbounded. For this reason, additional assertions are made to keep these
/// variables within their domains.
pub fn load_model_impl(
    store: &mut Store,
    solver: &mut Solver,
    theory_config: &TheoryConfig,
    symbols: &SymbolTable,
    model: &[Expression],
) -> Result<(), SolverError> {
    for (name, decl) in symbols.clone().into_iter_local() {
        let Some(var) = decl.as_var() else {
            continue;
        };
        let (ast, restriction) = var_to_ast(&name, &var, theory_config)?;
        store.insert(name, ast);
        solver.assert(restriction);
    }
    for expr in model.iter() {
        let bool: Bool = expr_to_ast(store, expr, theory_config)?;
        solver.assert(bool);
    }
    Ok(())
}

/// Returns the AST representation of the variable as well as a boolean assertion to restrict
/// it to the input variable's domain (e.g. integers have unbounded domains in SMT).
fn var_to_ast(
    name: &Name,
    var: &DecisionVariable,
    theories: &TheoryConfig,
) -> Result<(Dynamic, Bool), SolverError> {
    let sym = name_to_symbol(name)?;
    match &var.domain {
        // Booleans of course have the same domain in SMT, so no restriction required
        Domain::Bool => Ok((Bool::new_const(sym).into(), Bool::from_bool(true))),

        // Return a disjunction of the restrictions each range of the domain enforces
        // I.e. `x: int(1, 3..5)` -> `or([x = 1, x >= 3 /\ x <= 5])`
        Domain::Int(ranges) => match theories.ints {
            IntTheory::Lia => {
                let sym_const = Int::new_const(sym);
                let restrictions_res: Result<Vec<_>, SolverError> = ranges
                    .iter()
                    .map(|range| int_range_to_int_restriction(&sym_const, range))
                    .collect();
                let restrictions = restrictions_res?;
                Ok((sym_const.into(), Bool::or(restrictions.as_slice())))
            }
            IntTheory::Bv => {
                let sym_const = BV::new_const(sym, BV_SIZE);
                let restrictions_res: Result<Vec<_>, SolverError> = ranges
                    .iter()
                    .map(|range| int_range_to_bv_restriction(&sym_const, range))
                    .collect();
                let restrictions = restrictions_res?;
                Ok((sym_const.into(), Bool::or(restrictions.as_slice())))
            }
        },

        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "domain kind for '{name}' not implemented: {}",
            var.domain
        ))),
    }
}

/// Returns a boolean expression restricting the given integer variable to the given range.
fn int_range_to_int_restriction(var: &Int, range: &Range<i32>) -> Result<Bool, SolverError> {
    match range {
        Range::Single(n) => Ok(var.eq(Int::from(*n))),
        Range::UnboundedL(r) => Ok(var.le(Int::from(*r))),
        Range::UnboundedR(l) => Ok(var.ge(Int::from(*l))),
        Range::Bounded(l, r) => Ok(Bool::and(&[var.ge(Int::from(*l)), var.le(Int::from(*r))])),
        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "range type not implemented: {range}"
        ))),
    }
}

/// Returns a boolean expression restricting the given bitvector variable to the given integer range.
fn int_range_to_bv_restriction(var: &BV, range: &Range<i32>) -> Result<Bool, SolverError> {
    match range {
        Range::Single(n) => Ok(var.eq(BV::from_i64(*n as i64, BV_SIZE))),
        Range::UnboundedL(r) => Ok(var.bvsle(BV::from_i64(*r as i64, BV_SIZE))),
        Range::UnboundedR(l) => Ok(var.bvsge(BV::from_i64(*l as i64, BV_SIZE))),
        Range::Bounded(l, r) => Ok(Bool::and(&[
            var.bvsge(BV::from_i64(*l as i64, BV_SIZE)),
            var.bvsle(BV::from_i64(*r as i64, BV_SIZE)),
        ])),
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
fn expr_to_ast<Out>(
    store: &Store,
    expr: &Expression,
    thr: &TheoryConfig,
) -> Result<Out, SolverError>
where
    Out: TryFrom<Dynamic, Error: std::fmt::Display>,
{
    use IntTheory::{Bv, Lia};

    // Some translations are only allowed in certain theories.
    // E.g. if using LIA for ints then Expression::Sum is allowed, where in BV it must be PairwiseSum.
    let ast = match (thr.ints, expr) {
        (_, Expression::Atomic(_, atom)) => atom_to_ast(thr, store, atom),

        // Equality is part of the SMT core theory (anything can be compared)
        // We do some extra work to return a clean error if the types are different
        (_, Expression::Eq(_, a, b)) => {
            checked_binary_op(thr, store, a, b, |a: Dynamic, b: Dynamic| {
                a.safe_eq(b)
                    .map_err(|err| SolverError::ModelInvalid(err.to_string()))
            })?
        }
        (_, Expression::Neq(_, a, b)) => {
            checked_binary_op(thr, store, a, b, |a: Dynamic, b: Dynamic| {
                a.safe_eq(b)
                    .map(|eq| eq.not())
                    .map_err(|err| SolverError::ModelInvalid(err.to_string()))
            })?
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

/// Converts an atom (literal or reference) into an AST node.
fn atom_to_ast(
    theory_config: &TheoryConfig,
    store: &Store,
    atom: &Atom,
) -> Result<Dynamic, SolverError> {
    match atom {
        Atom::Reference(decl) => store
            .get(&decl.name())
            .ok_or(SolverError::ModelInvalid(format!(
                "variable '{}' does not exist",
                decl.name()
            )))
            .cloned(),
        Atom::Literal(lit) => literal_to_ast(theory_config, lit),
        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "atom sort not implemented: {atom}"
        ))),
    }
}

/// Converts a CO literal (expression containing no variables) into an AST node.
fn literal_to_ast(theory_config: &TheoryConfig, lit: &Literal) -> Result<Dynamic, SolverError> {
    match lit {
        Literal::Bool(b) => Ok(Bool::from_bool(*b).into()),
        Literal::Int(n) => Ok(match theory_config.ints {
            IntTheory::Lia => Int::from(*n).into(),
            IntTheory::Bv => BV::from_i64(*n as i64, BV_SIZE).into(),
        }),
        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "literal type not implemented: {lit}"
        ))),
    }
}

/// Interprets an expression as an AST and returns the result of the given operation over it.
fn unary_op<A, Out>(
    theories: &TheoryConfig,
    store: &Store,
    a: &Expression,
    op: impl FnOnce(A) -> Out,
) -> Result<Dynamic, SolverError>
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
    let a_ast: A = expr_to_ast(store, a, theories)?;
    let b_ast: B = expr_to_ast(store, b, theories)?;
    Ok((op)(a_ast, b_ast).into())
}

/// Interprets two expressions as ASTs and returns the result of the given operation over
/// them (which may be an error).
fn checked_binary_op<A, B, Error, Out>(
    theories: &TheoryConfig,
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
    let a_ast: A = expr_to_ast(store, a, theories)?;
    let b_ast: B = expr_to_ast(store, b, theories)?;
    Ok((op)(a_ast, b_ast).map(Into::into))
}

/// Transforms a slice of expressions into ASTs and returns the result of the given operation over it.
fn list_op<A, Out>(
    theories: &TheoryConfig,
    store: &Store,
    expr: &Expression,
    op: impl FnOnce(&[A]) -> Out,
) -> Result<Dynamic, SolverError>
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

    // Result implements FromIter, collecting into either the full collection or an error
    let asts_res: Result<Vec<_>, SolverError> = exprs
        .iter()
        .map(|e| expr_to_ast(store, e, theories))
        .collect();
    let asts = asts_res?;

    Ok((op)(asts.as_slice()).into())
}

//! Functions for converting from a Conjure Model to assertions in Z3.
//!
//! Conversions are mostly 1-to-1 since any rewriting was done previously using rules.
//! We recursively transform the AST bottom-up, returning the requested AST type (e.g. Bool, Int)
//! or an error.
//!
//! "Dynamic" or "AST" is used to describe generic Z3 AST values. "Expression" means
//! a Conjure Oxide [`Expression`] type.

use std::collections::HashSet;

use itertools::Itertools;
use z3::ast::*;
use z3::{Solver, Sort, SortKind, Symbol};

use super::IntTheory;
use super::helpers::*;
use super::store::SymbolStore;

use crate::ast::*;
use crate::solver::{SolverError, SolverResult};
use crate::{Model, bug};

/// Converts the given variables and constraints to assertions by mutating the given model.
///
/// SMT does not use bounded domains the same way Conjure Oxide does; for example integers
/// domains are unbounded. For this reason, additional assertions are made to keep these
/// variables within their domains.
pub fn load_model_impl(
    store: &mut SymbolStore,
    solver: &mut Solver,
    symbols: &SymbolTable,
    model: &[Expression],
) -> SolverResult<()> {
    for (name, decl) in symbols.clone().into_iter_local() {
        let Some(var) = decl.as_find() else {
            // Ignore lettings, etc
            continue;
        };
        if !decl.reprs().is_empty() {
            // This variable has a structured representation; ignore the source variable.
            continue;
        }
        if !symbols
            .representations_for(&name)
            .is_none_or(|reps| reps.is_empty())
        {
            // This variable has representations; ignore it
            continue;
        }
        let (sym, ast, restriction) = var_to_ast(&name, &var, int_theory_for(&decl))?;
        store.insert(name, (decl.resolved_domain().unwrap(), ast, sym));
        solver.assert(restriction);
    }
    for expr in model.iter() {
        let bool: Bool = expr_to_ast(store, expr)?;
        solver.assert(bool);
    }
    Ok(())
}

/// The integer theory a declaration's variable should be created in.
///
/// The `lia` and `bv` representations do not decompose the variable -- they leave one variable of
/// the same domain -- so the choice they record is read back here, off the declaration they were
/// initialised for. Anything with no such representation gets the default theory.
fn int_theory_for(decl: &DeclarationPtr) -> IntTheory {
    let source = decl.source().clone();
    let Some(source) = source else {
        return IntTheory::default();
    };
    source
        .reprs()
        .iter()
        .find_map(|(_, state)| IntTheory::from_repr_short_name(state.rule().short_name()))
        .unwrap_or_default()
}

/// Returns the AST representation of the variable as well as a boolean assertion which restricts
/// it to the input variable's domain since most Z3 sorts are unbounded.
fn var_to_ast(
    name: &Name,
    var: &DecisionVariable,
    ints: IntTheory,
) -> SolverResult<(Symbol, Dynamic, Bool)> {
    let sym = name_to_symbol(name)?;
    let dom = var
        .domain_of()
        .resolve()
        .unwrap_or_else(|e| bug!("could not resolve domain for {name}: {e}"));
    let (sort, restrict_fn) = domain_to_sort(dom.as_ref(), ints)?;
    let new_const = Dynamic::new_const(sym.clone(), &sort);

    let restriction = (restrict_fn)(&new_const);
    Ok((sym, new_const, restriction))
}

/// Converts a Conjure Oxide Expression to an AST node for Z3.
/// The generic type parameter lets us cast the result to a specific return type.
fn expr_to_ast<Out>(store: &SymbolStore, expr: &Expression) -> SolverResult<Out>
where
    Out: TryFrom<Dynamic, Error: std::fmt::Display>,
{
    let ast = match expr {
        Expression::Atomic(_, atom) => atom_to_ast(store, atom),

        // Equality is part of the SMT core theory (anything can be compared)
        // Some types (matrices, sets) must be compared element-wise, since the SMT solver can
        //  always extend them to make them technically eq/neq in "SMT land", but not in "Oxide land"
        // This is done during rewriting by rules which unwrap Eq/Neqs over these types
        //
        // The two sides can be in different integer theories -- that is what a `lia`/`bv`
        // channelling constraint looks like -- so they are brought into a common one first.
        Expression::Eq(_, a, b) => {
            let [a, b] = aligned_operands(store, [a.as_ref(), b.as_ref()])?;
            Ok(a.eq(b).into())
        }
        Expression::Neq(_, a, b) => {
            let [a, b] = aligned_operands(store, [a.as_ref(), b.as_ref()])?;
            Ok(a.ne(b).into())
        }

        // === Boolean Expressions ===
        Expression::Not(_, a) => unary_op(store, a, |a: Bool| a.not()),
        Expression::Imply(_, a, b) => binary_op(store, a, b, |a: Bool, b: Bool| a.implies(b)),
        Expression::Iff(_, a, b) => binary_op(store, a, b, |a: Bool, b: Bool| a.iff(b)),
        Expression::Or(_, a) => list_op(store, a, |asts: &[Bool]| Bool::or(asts)),
        Expression::And(_, a) => list_op(store, a, |asts: &[Bool]| Bool::and(asts)),

        // === Expressions over integers ===
        // Which theory each of these lands in is decided by the operands, not by a setting: a
        // variable represented as a bit-vector drags the operation into the bit-vector theory,
        // and everything else in the operation is converted to match.
        Expression::Neg(_, a) => {
            int_unary_op(store, a, |a: Int| a.unary_minus(), |a: BV| a.bvneg())
        }
        Expression::Abs(_, a) => int_unary_op(
            store,
            a,
            |a: Int| a.lt(Int::from(0)).ite(&a.unary_minus(), &a),
            |a: BV| a.bvslt(BV::from_i64(0, BV_SIZE)).ite(&a.bvneg(), &a),
        ),
        // A Boolean has no theory of its own to follow, so this always produces a mathematical
        // integer; an enclosing bit-vector operation converts it along with everything else.
        Expression::ToInt(_, a) => {
            unary_op(store, a, |a: Bool| a.ite(&Int::from(1), &Int::from(0)))
        }

        // Essence division floors, which is what Z3's integer `div` and `mod` already do. The
        // bit-vector theory offers truncating division instead (`bvsdiv` rounds toward zero, and
        // `bvsrem` takes the dividend's sign), so the two theories would disagree on negative
        // operands unless the floored forms are built explicitly.
        Expression::SafeDiv(_, a, b) => {
            int_binary_op(store, a, b, |a: Int, b: Int| a.div(b), bv_floor_div)
        }
        Expression::SafeMod(_, a, b) => {
            // `bvsmod` already takes the divisor's sign, which is the floored remainder.
            int_binary_op(
                store,
                a,
                b,
                |a: Int, b: Int| a.modulo(b),
                |a: BV, b: BV| a.bvsmod(b),
            )
        }
        Expression::Gt(_, a, b) => int_binary_op(
            store,
            a,
            b,
            |a: Int, b: Int| a.gt(b),
            |a: BV, b: BV| a.bvsgt(b),
        ),
        Expression::Lt(_, a, b) => int_binary_op(
            store,
            a,
            b,
            |a: Int, b: Int| a.lt(b),
            |a: BV, b: BV| a.bvslt(b),
        ),
        Expression::Geq(_, a, b) => int_binary_op(
            store,
            a,
            b,
            |a: Int, b: Int| a.ge(b),
            |a: BV, b: BV| a.bvsge(b),
        ),
        Expression::Leq(_, a, b) => int_binary_op(
            store,
            a,
            b,
            |a: Int, b: Int| a.le(b),
            |a: BV, b: BV| a.bvsle(b),
        ),
        Expression::PairwiseSum(_, a, b) => int_binary_op(
            store,
            a,
            b,
            |a: Int, b: Int| Int::add(&[a, b]),
            |a: BV, b: BV| a.bvadd(b),
        ),
        Expression::PairwiseProduct(_, a, b) => int_binary_op(
            store,
            a,
            b,
            |a: Int, b: Int| Int::mul(&[a, b]),
            |a: BV, b: BV| a.bvmul(b),
        ),

        // Bit-vectors have no n-ary addition, so the list is folded pairwise here rather than
        // being rewritten into `PairwiseSum` chains beforehand.
        Expression::Sum(_, a) => int_list_op(
            store,
            a,
            |asts: &[Int]| Int::add(asts),
            |asts: &[BV]| fold_bvs(asts, |a, b| a.bvadd(b)),
        ),
        Expression::Product(_, a) => int_list_op(
            store,
            a,
            |asts: &[Int]| Int::mul(asts),
            |asts: &[BV]| fold_bvs(asts, |a, b| a.bvmul(b)),
        ),

        // Exponentiation has no bit-vector counterpart in Z3, so this one stays integer-only.
        Expression::SafePow(_, a, b) => binary_op(store, a, b, |a: Int, b: Int| a.power(b)),

        // === Expressions involving matrices ===
        // A matrix literal becomes a Z3 array, which is what makes indexing one by a variable
        // work. Components-represented matrices are rebuilt as literal lists during rewriting, so
        // this is the path a variable index into them takes.
        Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elements, index_domain)) => {
            matrix_literal_to_array(store, elements, index_domain)
        }
        Expression::SafeIndex(_, m, idxs) => {
            let arr: Dynamic = expr_to_ast(store, m)?;
            slice_op(store, idxs, move |idxs: &[Dynamic]| {
                idxs.iter().fold(arr, |cur_arr, idx| {
                    // Array indices are always in the integer theory, whatever the elements are.
                    let idx = coerce_to(idx.clone(), Some(IntTheory::Lia));
                    cur_arr.as_array().unwrap().select(&idx)
                })
            })
        }
        // `allDifferent` itself never reaches here: the rules turn it into either this native
        // form or an explicit encoding, and which one is a modelling choice.
        Expression::SmtDistinct(_, a) => {
            let elements = list_elements(a)?;
            let refs: Vec<&Expression> = elements.iter().collect();
            let asts = aligned_operand_vec(store, &refs)?;
            Ok(Dynamic::distinct(&asts).into())
        }

        // === Expressions involving sets
        Expression::In(_, x, s) => binary_op(store, x, s, |x: Dynamic, s: Set| s.member(&x)),

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

/// Builds a Z3 array holding the elements of a matrix literal at their index-domain positions.
///
/// Entries outside the literal keep the array's default value; the model only ever reads positions
/// the index domain allows, which `SafeIndex` has already restricted by this point.
fn matrix_literal_to_array(
    store: &SymbolStore,
    elements: &[Expression],
    index_domain: &DomainPtr,
) -> SolverResult<Dynamic> {
    let asts: Vec<Dynamic> = elements
        .iter()
        .map(|element| expr_to_ast(store, element))
        .collect::<SolverResult<_>>()?;

    let Some(_) = asts.first() else {
        return Err(SolverError::ModelFeatureNotImplemented(
            "cannot build an array from an empty matrix literal".to_owned(),
        ));
    };

    // Elements can come from declarations represented in different theories; give the array one
    // element sort and bring them all to it, exactly as an operation over them would.
    let target = asts
        .iter()
        .filter_map(int_theory_of)
        .fold(None, |acc, theory| match (acc, theory) {
            (Some(IntTheory::Bv), _) | (_, IntTheory::Bv) => Some(IntTheory::Bv),
            _ => Some(IntTheory::Lia),
        });
    let asts: Vec<Dynamic> = asts.into_iter().map(|ast| coerce_to(ast, target)).collect();

    // A plain list is written with an implied, unbounded index domain (`int(1..)`), which cannot
    // be enumerated; its positions are simply one per element.
    let indices = index_domain
        .resolve()
        .ok()
        .and_then(|domain| domain_to_ast_vec(IntTheory::Lia, domain.as_ref()).ok())
        .filter(|indices| indices.len() == asts.len())
        .unwrap_or_else(|| {
            (1..=asts.len())
                .map(|position| Int::from(position as i64).into())
                .collect()
        });

    let index_sort = indices
        .first()
        .map(|index| index.get_sort())
        .unwrap_or_else(Sort::int);

    let mut array = Array::const_array(&index_sort, &asts[0]);
    for (index, value) in indices.iter().zip(asts.iter()) {
        array = array.store(index, value);
    }
    Ok(array.into())
}

/// Floored bit-vector division, matching Essence's `/` and Z3's integer `div`.
///
/// `bvsdiv` truncates toward zero, so it is one too high whenever the operands have opposite signs
/// and the division is inexact; `bvsmod` takes the divisor's sign, so a non-zero remainder is
/// exactly the case needing the correction.
fn bv_floor_div(a: BV, b: BV) -> BV {
    let zero = BV::from_i64(0, BV_SIZE);
    let one = BV::from_i64(1, BV_SIZE);
    let truncated = a.bvsdiv(&b);
    let remainder = a.bvsrem(&b);
    let signs_differ = a.bvslt(&zero).xor(b.bvslt(&zero));
    let inexact = remainder.eq(&zero).not();
    Bool::and(&[signs_differ, inexact]).ite(&truncated.bvsub(&one), &truncated)
}

/// Folds a non-empty slice of bit-vectors with a binary operation.
fn fold_bvs(asts: &[BV], op: impl Fn(BV, BV) -> BV) -> BV {
    asts.iter()
        .cloned()
        .reduce(op)
        .unwrap_or_else(|| BV::from_i64(0, BV_SIZE))
}

/// The integer theory an already-converted AST is in, if it is an integer at all.
fn int_theory_of(ast: &Dynamic) -> Option<IntTheory> {
    match ast.sort_kind() {
        SortKind::Int => Some(IntTheory::Lia),
        SortKind::Bv => Some(IntTheory::Bv),
        _ => None,
    }
}

/// Converts an integer AST into `theory`, leaving non-integers and already-matching ASTs alone.
///
/// This is what makes a `lia`/`bv` channelling constraint expressible, and what lets an operation
/// take operands whose declarations were represented in different theories.
fn coerce_to(ast: Dynamic, theory: Option<IntTheory>) -> Dynamic {
    match (theory, ast.sort_kind()) {
        (Some(IntTheory::Bv), SortKind::Int) => {
            BV::from_int(&ast.as_int().expect("just checked the sort"), BV_SIZE).into()
        }
        (Some(IntTheory::Lia), SortKind::Bv) => {
            Int::from_bv(&ast.as_bv().expect("just checked the sort"), true).into()
        }
        _ => ast,
    }
}

/// Converts every operand and brings any integers among them into a common theory.
///
/// Bit-vectors win: converting a mathematical integer to a machine word is exact for values that
/// fit, whereas the reverse would have to pick a signedness for values that may have wrapped.
fn aligned_operand_vec(store: &SymbolStore, exprs: &[&Expression]) -> SolverResult<Vec<Dynamic>> {
    let asts: Vec<Dynamic> = exprs
        .iter()
        .map(|expr| expr_to_ast(store, expr))
        .collect::<SolverResult<_>>()?;

    let target = asts
        .iter()
        .filter_map(int_theory_of)
        .fold(None, |acc, theory| match (acc, theory) {
            (Some(IntTheory::Bv), _) | (_, IntTheory::Bv) => Some(IntTheory::Bv),
            _ => Some(IntTheory::Lia),
        });

    Ok(asts.into_iter().map(|ast| coerce_to(ast, target)).collect())
}

/// [`aligned_operand_vec`] for a fixed number of operands.
fn aligned_operands<const N: usize>(
    store: &SymbolStore,
    exprs: [&Expression; N],
) -> SolverResult<[Dynamic; N]> {
    let refs: Vec<&Expression> = exprs.to_vec();
    let asts = aligned_operand_vec(store, &refs)?;
    asts.try_into().map_err(|_| {
        SolverError::ModelInvalid("wrong number of operands after alignment".to_owned())
    })
}

/// Applies whichever of the two operations matches the theory the operand landed in.
fn int_unary_op<OutInt, OutBv>(
    store: &SymbolStore,
    a: &Expression,
    lia: impl FnOnce(Int) -> OutInt,
    bv: impl FnOnce(BV) -> OutBv,
) -> SolverResult<Dynamic>
where
    OutInt: Into<Dynamic>,
    OutBv: Into<Dynamic>,
{
    let ast: Dynamic = expr_to_ast(store, a)?;
    match int_theory_of(&ast) {
        Some(IntTheory::Bv) => Ok((bv)(ast.as_bv().expect("just checked the sort")).into()),
        _ => Ok((lia)(ast.as_int().ok_or_else(|| {
            SolverError::ModelInvalid(format!("expected an integer operand: {a}"))
        })?)
        .into()),
    }
}

/// Applies whichever of the two operations matches the theory the operands were aligned into.
fn int_binary_op<OutInt, OutBv>(
    store: &SymbolStore,
    a: &Expression,
    b: &Expression,
    lia: impl FnOnce(Int, Int) -> OutInt,
    bv: impl FnOnce(BV, BV) -> OutBv,
) -> SolverResult<Dynamic>
where
    OutInt: Into<Dynamic>,
    OutBv: Into<Dynamic>,
{
    let [a_ast, b_ast] = aligned_operands(store, [a, b])?;
    match int_theory_of(&a_ast) {
        Some(IntTheory::Bv) => Ok((bv)(
            a_ast.as_bv().expect("just checked the sort"),
            b_ast.as_bv().expect("aligned with the first operand"),
        )
        .into()),
        _ => {
            let cast = |ast: &Dynamic, expr: &Expression| {
                ast.as_int().ok_or_else(|| {
                    SolverError::ModelInvalid(format!("expected an integer operand: {expr}"))
                })
            };
            Ok((lia)(cast(&a_ast, a)?, cast(&b_ast, b)?).into())
        }
    }
}

/// Applies whichever of the two operations matches the theory the list was aligned into.
fn int_list_op<OutInt, OutBv>(
    store: &SymbolStore,
    expr: &Expression,
    lia: impl FnOnce(&[Int]) -> OutInt,
    bv: impl FnOnce(&[BV]) -> OutBv,
) -> SolverResult<Dynamic>
where
    OutInt: Into<Dynamic>,
    OutBv: Into<Dynamic>,
{
    let elements = list_elements(expr)?;
    let refs: Vec<&Expression> = elements.iter().collect();
    let asts = aligned_operand_vec(store, &refs)?;

    if asts
        .iter()
        .any(|ast| matches!(int_theory_of(ast), Some(IntTheory::Bv)))
    {
        let bvs: Vec<BV> = asts
            .iter()
            .map(|ast| ast.as_bv().expect("the whole list was aligned"))
            .collect();
        return Ok((bv)(&bvs).into());
    }

    let ints: Vec<Int> = asts
        .iter()
        .map(|ast| {
            ast.as_int().ok_or_else(|| {
                SolverError::ModelInvalid(format!("expected integer operands: {expr}"))
            })
        })
        .collect::<SolverResult<_>>()?;
    Ok((lia)(&ints).into())
}

/// Interprets an expression as an AST and returns the result of the given operation over it.
fn unary_op<A, Out>(
    store: &SymbolStore,
    a: &Expression,
    op: impl FnOnce(A) -> Out,
) -> SolverResult<Dynamic>
where
    A: TryFrom<Dynamic, Error: std::fmt::Display>,
    Out: Into<Dynamic>,
{
    let a_ast: A = expr_to_ast(store, a)?;
    Ok((op)(a_ast).into())
}

/// Interprets two expressions as ASTs and returns the result of the given operation over them.
fn binary_op<A, B, Out>(
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
    let a_ast: A = expr_to_ast(store, a)?;
    let b_ast: B = expr_to_ast(store, b)?;
    Ok((op)(a_ast, b_ast).into())
}

/// Transforms a list expression into separate ASTs and returns the result of the given operation over them.
fn list_op<A, Out>(
    store: &SymbolStore,
    expr: &Expression,
    op: impl FnOnce(&[A]) -> Out,
) -> SolverResult<Dynamic>
where
    A: TryFrom<Dynamic, Error: std::fmt::Display>,
    Out: Into<Dynamic>,
{
    let exprs = list_elements(expr)?;

    slice_op(store, &exprs, op)
}

/// Extracts the elements of a list-like expression.
///
/// Besides literal lists, this also supports slice expressions such as `x[..]`
/// by expanding them to explicit indexing expressions.
/// TODO: Consider moving this out of the smt solver adaptor, it'll be more generally useful.
fn list_elements(expr: &Expression) -> SolverResult<Vec<Expression>> {
    if let Some(exprs) = expr.clone().unwrap_list() {
        return Ok(exprs);
    }

    // `unwrap_list` only recognises a matrix whose index domain is the implied `int(1..)`. The
    // operations that reach here -- sums, conjunctions, `distinct` -- care about the elements and
    // not where they are indexed from, and a matrix rebuilt from its components keeps the index
    // domain it was declared with, so take those elements too.
    if let Expression::AbstractLiteral(_, AbstractLiteral::Matrix(elements, _)) = expr {
        return Ok(elements.clone());
    }

    let Expression::SafeSlice(_, subject, indices) = expr else {
        return Err(SolverError::ModelFeatureNotImplemented(format!(
            "inner expression must be a list: {expr}"
        )));
    };

    let subject_domain = subject
        .domain_of()
        .and_then(|d| d.resolve().ok())
        .ok_or_else(|| {
            SolverError::ModelFeatureNotImplemented(format!(
                "cannot resolve domain of sliced expression: {expr}"
            ))
        })?;

    let GroundDomain::Matrix(_, index_domains) = subject_domain.as_ref() else {
        return Err(SolverError::ModelFeatureNotImplemented(format!(
            "slice subject must have matrix domain: {expr}"
        )));
    };

    if index_domains.len() != indices.len() {
        return Err(SolverError::ModelInvalid(format!(
            "slice has wrong number of indices: {expr}"
        )));
    }

    let index_options: SolverResult<Vec<Vec<Expression>>> = indices
        .iter()
        .zip(index_domains.iter())
        .map(|(idx, dom)| match idx {
            Some(idx_expr) => Ok(vec![idx_expr.clone()]),
            None => {
                let vals = dom.values().map_err(|_| {
                    SolverError::ModelFeatureNotImplemented(format!(
                        "slice index domain is not finite/enumerable: {dom}"
                    ))
                })?;
                Ok(vals
                    .map(|lit| Expression::Atomic(Metadata::new(), Atom::Literal(lit)))
                    .collect_vec())
            }
        })
        .collect();

    let index_options = index_options?;

    Ok(index_options
        .into_iter()
        .multi_cartesian_product()
        .map(|concrete_idxs| Expression::SafeIndex(Metadata::new(), subject.clone(), concrete_idxs))
        .collect())
}

/// Transforms a slice of expressions into ASTs and returns the result of the given operation over it.
fn slice_op<A, Out>(
    store: &SymbolStore,
    exprs: &[Expression],
    op: impl FnOnce(&[A]) -> Out,
) -> SolverResult<Dynamic>
where
    A: TryFrom<Dynamic, Error: std::fmt::Display>,
    Out: Into<Dynamic>,
{
    // Result implements FromIter, collecting into either the full collection or an error
    let asts_res: SolverResult<Vec<_>> = exprs.iter().map(|e| expr_to_ast(store, e)).collect();
    let asts = asts_res?;

    Ok((op)(asts.as_slice()).into())
}

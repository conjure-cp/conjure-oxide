use super::TupleComponents;
use crate::guard;
use crate::shared::utils::{
    as_cmp_or_lex_op, as_eq_or_neq, collect_cmp_exprs, collect_eq_or_neq, tuple_expr_entries,
};
use conjure_cp::ast::{Atom, Expression as Expr, Literal, Metadata, Reference, SymbolTable};
use conjure_cp::bug_assert;
use conjure_cp::bug_assert_eq;
use conjure_cp::rule_engine::ApplicationError::RuleNotApplicable;
use conjure_cp::rule_engine::{ApplicationResult, RuleEffect as Reduction, register_rule};
use itertools::izip;

/// Indexing into a tuple variable
/// ```plain
/// x[1]
/// ~>
/// x_TupleComponents_1
/// ```
#[register_rule("ReprGeneral", 9500, [SafeIndex])]
fn tuple_components_index_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::SafeIndex(_, subject, indices) = expr        &&
        let Expr::Atomic(_, Atom::Reference(re)) = &**subject  &&
        let Some(Expr::Atomic(_, idx)) = indices.first()       &&
        let Atom::Literal(Literal::Int(idx)) = idx             &&
        let Some(repr) = re.get_repr_as::<TupleComponents>()
        else {
            return Err(RuleNotApplicable);
        }
    );
    let idx = (*idx - 1) as usize;
    bug_assert!(idx < repr.elems.len(), "tuple indexing is out of bounds");

    let lhs = Reference::new(repr.elems[idx].clone());
    let rhs = &indices[1..];

    if rhs.is_empty() {
        Ok(Reduction::pure(lhs.into()))
    } else {
        let new_expr = Expr::SafeIndex(Metadata::new(), lhs.into(), Vec::from(rhs));
        Ok(Reduction::pure(new_expr))
    }
}

/// Equality of tuple variables
/// ```plain
/// x = y
/// ~>
/// x[1] = y[1] /\ x[2] = y[2] /\ ... /\ x[N] = y[N]
/// ```
#[register_rule("ReprGeneral", 9400, [Eq, Neq])]
fn tuple_var_eq_var(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;

    guard!(
        let Expr::Atomic(_, Atom::Reference(re)) = lhs     &&
        let Some(repr) = re.get_repr_as::<TupleComponents>()   &&
        let Expr::Atomic(_, Atom::Reference(re2)) = rhs    &&
        let Some(repr2) = re2.get_repr_as::<TupleComponents>()
        else {
            return Err(RuleNotApplicable);
        }
    );

    bug_assert_eq!(
        repr.elems.len(),
        repr2.elems.len(),
        "equality on tuples with different shapes!"
    );

    let new_expr = collect_eq_or_neq(neq, izip!(repr.elem_refs(), repr2.elem_refs()));
    Ok(Reduction::pure(new_expr))
}

/// Equality of tuple variable to a tuple literal
/// ```plain
/// x = (1, true)
/// ~>
/// x[1] = 1 /\ x[2] = true
/// ```
#[register_rule("ReprGeneral", 9400, [Eq, Neq])]
fn tuple_var_eq_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;

    guard!(
        let Expr::Atomic(_, Atom::Reference(re)) = lhs   &&
        let Some(repr) = re.get_repr_as::<TupleComponents>() &&
        let Some(rhs_ents) = tuple_expr_entries(rhs)
        else {
            return Err(RuleNotApplicable);
        }
    );

    bug_assert_eq!(
        repr.elems.len(),
        rhs_ents.len(),
        "equality on tuples with different shapes!"
    );

    let new_expr = collect_eq_or_neq(neq, izip!(repr.elem_refs(), rhs_ents));
    Ok(Reduction::pure(new_expr))
}

/// Comparison operations on tuple variables;
/// comparisons are done in the order of the fields
/// ```plain
/// x > y
/// ~>
/// or([
///   x[1] > y[1],
///   x[1] = y[1] /\ x[2] > y[2],
///   ...
///   x[1] = y[1] /\ x[2] = y[2] /\ ... /\ x[n-1] = y[n-1] /\ x[n] > y[n]
/// ])
/// ```
#[register_rule(
    "ReprGeneral",
    9400,
    [Lt, Gt, Leq, Geq, LexLt, LexGt, LexLeq, LexGeq]
)]
fn tuple_var_cmp_var(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        as_eq_or_neq(expr).is_err() && // equality handled separately
        let Some((lhs, rhs)) = as_cmp_or_lex_op(expr)               &&
        let Expr::Atomic(_, Atom::Reference(lhs_re)) = lhs.as_ref() &&
        let Expr::Atomic(_, Atom::Reference(rhs_re)) = rhs.as_ref() &&
        let Some(lhs_repr) = lhs_re.get_repr_as::<TupleComponents>()    &&
        let Some(rhs_repr) = rhs_re.get_repr_as::<TupleComponents>()
        else {
            return Err(RuleNotApplicable);
        }
    );

    let new_expr = collect_cmp_exprs(expr, lhs_repr.field_exprs(), rhs_repr.field_exprs());
    Ok(Reduction::pure(new_expr))
}

/// Comparison operations on tuple variables;
/// comparisons are done in the order of the fields
/// ```plain
/// x > (1, 2, 3)
/// ~>
/// or([
///   x[1] > 1,
///   x[1] = 1 /\ x[2] > 2,
///   x[1] = 1 /\ x[2] = 2 /\ x[3] > 3
/// ])
/// ```
#[register_rule(
    "ReprGeneral",
    9400,
    [Lt, Gt, Leq, Geq, LexLt, LexGt, LexLeq, LexGeq]
)]
fn tuple_var_cmp_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        as_eq_or_neq(expr).is_err() && // equality handled separately
        let Some((lhs, rhs)) = as_cmp_or_lex_op(expr)               &&
        let Expr::Atomic(_, Atom::Reference(lhs_re)) = lhs.as_ref() &&
        let Some(lhs_repr) = lhs_re.get_repr_as::<TupleComponents>()    &&
        let Some(rhs_ents) = tuple_expr_entries(&rhs)
        else {
            return Err(RuleNotApplicable);
        }
    );

    let new_expr = collect_cmp_exprs(expr, lhs_repr.field_exprs(), rhs_ents);
    Ok(Reduction::pure(new_expr))
}

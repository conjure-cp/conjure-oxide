use super::TuplePacked;
use crate::guard;
use crate::shared::utils::{
    as_cmp_or_lex_op, as_eq_or_neq, collect_cmp_exprs, collect_eq_or_neq, eq_or_neq,
    tuple_expr_entries,
};
use crate::types::tuple::TupleComponents;
use conjure_cp::ast::{Atom, Expression as Expr, Literal, Metadata, Moo, Reference, SymbolTable};
use conjure_cp::bug_assert;
use conjure_cp::essence_expr;
use conjure_cp::representation::ReprRule;
use conjure_cp::rule_engine::ApplicationError::RuleNotApplicable;
use conjure_cp::rule_engine::{
    ApplicationResult, RuleEffect as Reduction, register_rule, register_rule_set,
};
use parking_lot::MappedRwLockReadGuard;

// Packed tuples are backend-neutral, so make their lowering rules available
// for every solver family.
register_rule_set!("ReprTuplePacked", ("Base"), |_| true);

/// Equality of packed tuple variables.
///
/// Matching layouts can be compared directly. Different layouts are decoded
/// and compared element-wise because the same packed value can represent
/// different tuples in each layout.
/// ```plain
/// x = y  (both TuplePacked)  ~>  x_packed = y_packed
/// ```
#[register_rule("ReprTuplePacked", 9700, [Eq, Neq])]
fn tuple_packed_var_eq_var(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;

    guard!(
        let Expr::Atomic(_, Atom::Reference(re))  = lhs &&
        let Some(lp) = re.get_repr_as::<TuplePacked>()  &&
        let Expr::Atomic(_, Atom::Reference(re2)) = rhs &&
        let Some(rp) = re2.get_repr_as::<TuplePacked>()
        else { return Err(RuleNotApplicable) }
    );

    bug_assert!(
        lp.sizes.len() == rp.sizes.len(),
        "equality on tuples with different shapes!"
    );

    let new_expr = if layouts_match(&lp, &rp) {
        eq_or_neq(neq, lp.packed_expr(), rp.packed_expr())
    } else {
        collect_eq_or_neq(
            neq,
            unpack_entries(&lp).into_iter().zip(unpack_entries(&rp)),
        )
    };
    Ok(Reduction::pure(new_expr))
}

/// Equality of packed tuple variable to a tuple literal
/// ```plain
/// x = (1, 2, 3)  (x is TuplePacked)  ~>  x_packed = encode(1,2,3)
/// ```
#[register_rule("ReprTuplePacked", 9700, [Eq, Neq])]
fn tuple_packed_var_eq_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;

    guard!(
        let Expr::Atomic(_, Atom::Reference(re)) = lhs   &&
        let Some(repr) = re.get_repr_as::<TuplePacked>() &&
        let Some(rhs_ents) = tuple_expr_entries(rhs)
        else { return Err(RuleNotApplicable) }
    );

    bug_assert!(
        repr.sizes.len() == rhs_ents.len(),
        "equality on tuples with different shapes!"
    );

    let packed_val = repr.encode_lit_entries(&rhs_ents)?;
    let new_expr = eq_or_neq(neq, repr.packed_expr(), packed_val);
    Ok(Reduction::pure(new_expr))
}

/// Comparison of packed tuple variables.
///
/// Matching layouts preserve lexicographic order directly. Different layouts
/// are decoded before applying the tuple comparison.
/// ```plain
/// x > y  (both TuplePacked)  ~>  x_packed > y_packed
/// ```
#[register_rule(
    "ReprTuplePacked",
    9700,
    [Lt, Gt, Leq, Geq, LexLt, LexGt, LexLeq, LexGeq]
)]
fn tuple_packed_var_cmp_var(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        as_eq_or_neq(expr).is_err() &&
        let Some((lhs, rhs)) = as_cmp_or_lex_op(expr)               &&
        let Expr::Atomic(_, Atom::Reference(lhs_re)) = lhs.as_ref() &&
        let Expr::Atomic(_, Atom::Reference(rhs_re)) = rhs.as_ref() &&
        let Some(lp) = lhs_re.get_repr_as::<TuplePacked>()          &&
        let Some(rp) = rhs_re.get_repr_as::<TuplePacked>()
        else { return Err(RuleNotApplicable) }
    );

    bug_assert!(
        lp.sizes.len() == rp.sizes.len(),
        "comparison of tuples with different shapes!"
    );

    let new_expr = if layouts_match(&lp, &rp) {
        packed_cmp(expr, lp.packed_expr(), rp.packed_expr())
    } else {
        collect_cmp_exprs(expr, unpack_entries(&lp), unpack_entries(&rp))
    };
    Ok(Reduction::pure(new_expr))
}

/// Comparison of packed tuple variable to a literal
/// ```plain
/// x > (1,2,3)  (x is TuplePacked)  ~>  x_packed > encode(1,2,3)
/// ```
#[register_rule(
    "ReprTuplePacked",
    9700,
    [Lt, Gt, Leq, Geq, LexLt, LexGt, LexLeq, LexGeq]
)]
fn tuple_packed_var_cmp_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        as_eq_or_neq(expr).is_err() &&
        let Some((lhs, rhs)) = as_cmp_or_lex_op(expr)               &&
        let Expr::Atomic(_, Atom::Reference(lhs_re)) = lhs.as_ref() &&
        let Some(repr) = lhs_re.get_repr_as::<TuplePacked>()        &&
        let Some(rhs_ents) = tuple_expr_entries(&rhs)
        else { return Err(RuleNotApplicable) }
    );

    let packed_val = repr.encode_lit_entries(&rhs_ents)?;
    Ok(Reduction::pure(packed_cmp(
        expr,
        repr.packed_expr(),
        packed_val,
    )))
}

/// Indexing into a packed tuple variable
/// ```plain
/// x[i]  (x is TuplePacked)  ~>  (x_packed / stride_i) % size_i + min_i
/// ```
#[register_rule("ReprTuplePacked", 9700, [SafeIndex])]
fn tuple_packed_index_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::SafeIndex(_, subject, indices) = expr       &&
        let Expr::Atomic(_, Atom::Reference(re)) = &**subject &&
        let Some(Expr::Atomic(_, idx)) = indices.first()      &&
        let Atom::Literal(Literal::Int(idx)) = idx            &&
        let Some(repr) = re.get_repr_as::<TuplePacked>()
        else { return Err(RuleNotApplicable) }
    );

    let i = (*idx - 1) as usize;
    bug_assert!(i < repr.sizes.len(), "tuple indexing is out of bounds");

    let new_expr = unpack_entry(&repr, i);

    let remaining = &indices[1..];
    if remaining.is_empty() {
        Ok(Reduction::pure(new_expr))
    } else {
        Ok(Reduction::pure(Expr::SafeIndex(
            Metadata::new(),
            new_expr.into(),
            Vec::from(remaining),
        )))
    }
}

/// Channeling constraint between TupleComponents and TuplePacked for the same variable.
/// Handles equalities of the form `x#TupleComponents = x#TuplePacked` (or reversed).
/// ```plain
/// x#TupleComponents = x#TuplePacked
/// ~>
/// x_packed = sum_i (x_TupleComponents_i - min_i) * stride_i
/// ```
#[register_rule("ReprTuplePacked", 9700, [Eq, Neq])]
fn tuple_channel_atom_packed(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;

    guard!(
        let Expr::Atomic(_, Atom::Reference(re_a)) = lhs          &&
        let Expr::Atomic(_, Atom::Reference(re_b)) = rhs          &&
        let Some((packed, atom)) = as_channeling_pair(re_a, re_b)
        else {
            return Err(RuleNotApplicable);
        }
    );

    // Build: packed = sum_i (atom_i - min_i) * stride_i
    let sum_expr = atom
        .elems
        .iter()
        .enumerate()
        .map(|(i, decl)| {
            let elem: Expr = Reference::new(decl.clone()).into();
            let offset = match packed.mins[i] {
                0 => elem,
                min_i => essence_expr!(&elem - &min_i),
            };
            match packed.strides[i] {
                1 => offset,
                stride_i => essence_expr!(&offset * &stride_i),
            }
        })
        .reduce(|acc: Expr, part: Expr| essence_expr!(&acc + &part))
        .unwrap();

    Ok(Reduction::pure(eq_or_neq(
        neq,
        packed.packed_expr(),
        sum_expr,
    )))
}

/// Build a scalar comparison matching the given (possibly lex) comparison operator.
/// Packed tuples are single integers, so `LexLt` → `Lt`, etc.
fn packed_cmp(op: &Expr, lhs: Expr, rhs: Expr) -> Expr {
    let (lhs, rhs) = (Moo::new(lhs), Moo::new(rhs));
    match op {
        Expr::Lt(..) | Expr::LexLt(..) => Expr::Lt(Metadata::new(), lhs, rhs),
        Expr::Leq(..) | Expr::LexLeq(..) => Expr::Leq(Metadata::new(), lhs, rhs),
        Expr::Gt(..) | Expr::LexGt(..) => Expr::Gt(Metadata::new(), lhs, rhs),
        Expr::Geq(..) | Expr::LexGeq(..) => Expr::Geq(Metadata::new(), lhs, rhs),
        _ => unreachable!("packed_cmp: unexpected operator"),
    }
}

fn layouts_match(lhs: &PackedState<'_>, rhs: &PackedState<'_>) -> bool {
    lhs.sizes == rhs.sizes && lhs.mins == rhs.mins
}

fn unpack_entries(repr: &PackedState<'_>) -> Vec<Expr> {
    (0..repr.sizes.len())
        .map(|index| unpack_entry(repr, index))
        .collect()
}

fn unpack_entry(repr: &PackedState<'_>, index: usize) -> Expr {
    let packed = repr.packed_expr();
    let stride = repr.strides[index];
    let size = repr.sizes[index];
    let min = repr.mins[index];

    // (packed / stride) % size + min
    match (stride, index) {
        (1, _) if size == repr.total_size => essence_expr!(&packed + &min),
        (1, _) => essence_expr!((&packed % &size) + &min),
        (_, 0) => essence_expr!((&packed / &stride) + &min),
        _ => essence_expr!(((&packed / &stride) % &size) + &min),
    }
}

type PackedState<'a> = MappedRwLockReadGuard<'a, <TuplePacked as ReprRule>::DeclLevel>;
type ComponentsState<'a> = MappedRwLockReadGuard<'a, <TupleComponents as ReprRule>::DeclLevel>;
fn as_channeling_pair<'a>(
    lhs: &'a Reference,
    rhs: &'a Reference,
) -> Option<(PackedState<'a>, ComponentsState<'a>)> {
    if lhs.ptr != rhs.ptr {
        return None;
    }
    let packed = match (
        lhs.get_repr_as::<TuplePacked>(),
        rhs.get_repr_as::<TuplePacked>(),
    ) {
        (Some(lhs), None) => lhs,
        (None, Some(rhs)) => rhs,
        _ => return None,
    };
    let atom = match (
        lhs.get_repr_as::<TupleComponents>(),
        rhs.get_repr_as::<TupleComponents>(),
    ) {
        (Some(lhs), None) => lhs,
        (None, Some(rhs)) => rhs,
        _ => return None,
    };
    Some((packed, atom))
}

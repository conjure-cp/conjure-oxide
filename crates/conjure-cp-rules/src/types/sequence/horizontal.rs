use crate::guard;
use crate::shared::utils::as_eq_or_neq;
use crate::types::sequence::{SequenceExplicit, SequencePacked};
use conjure_cp::ast::{
    AbstractLiteral, Atom, Expression as Expr, Literal, Metadata, Moo, SymbolTable,
};
use conjure_cp::rule_engine::ApplicationError::RuleNotApplicable;
use conjure_cp::rule_engine::{ApplicationResult, RuleEffect as Reduction, register_rule};
use conjure_cp::{into_matrix_expr, matrix_expr};
use itertools::Itertools;

/// A uniform, representation-independent view over a sequence-typed expression: a literal, or a
/// variable represented by [`SequenceExplicit`] or [`SequencePacked`].
///
/// Positions are one-based; `slot[i - 1]` is the value expression at position `i`, for every `i`
/// in `1..=max_size`. `length_expr` is the active length (a constant for a literal or fixed-size
/// representation, otherwise a reference to the length marker/digit).
struct SeqSide {
    max_size: i32,
    length_expr: Expr,
    slot: Vec<Expr>,
}

fn sequence_expr_entries(expr: &Expr) -> Option<Vec<Expr>> {
    match expr {
        Expr::AbstractLiteral(_, AbstractLiteral::Sequence(elems)) => Some(elems.clone()),
        Expr::Atomic(
            _,
            Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Sequence(elems))),
        ) => Some(elems.iter().cloned().map(Expr::from).collect()),
        _ => None,
    }
}

fn sequence_side(expr: &Expr) -> Option<SeqSide> {
    if let Some(elems) = sequence_expr_entries(expr) {
        let max_size = elems.len() as i32;
        return Some(SeqSide {
            max_size,
            length_expr: max_size.into(),
            slot: elems,
        });
    }
    let Expr::Atomic(_, Atom::Reference(re)) = expr else {
        return None;
    };
    if let Some(repr) = re.get_repr_as::<SequenceExplicit>() {
        let (_, max) = repr.size_bounds;
        return Some(SeqSide {
            max_size: max,
            length_expr: repr.length_expr(),
            slot: (1..=max).map(|i| repr.slot_expr(i)).collect(),
        });
    }
    if let Some(repr) = re.get_repr_as::<SequencePacked>() {
        let (_, max) = repr.size_bounds;
        return Some(SeqSide {
            max_size: max,
            length_expr: repr.length_expr(),
            slot: (1..=max).map(|i| repr.slot_expr(i)).collect(),
        });
    }
    None
}

fn is_reference(expr: &Expr) -> bool {
    matches!(expr, Expr::Atomic(_, Atom::Reference(_)))
}

/// Equality between two sequences (at least one represented by a variable).
///
/// Positions beyond the shorter side's own maximum size are unreachable once the lengths are
/// forced equal, so only the common prefix needs comparing.
/// ```plain
/// x = y
/// ~>
/// |x| = |y| /\ and([ i > |x| \/ x[i] = y[i] | i : int(1..min(xMaxSize, yMaxSize)) ])
/// ```
#[register_rule("ReprGeneral", 9500, [Eq, Neq])]
fn sequence_side_eq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;

    guard!(
        let Some(lhs_side) = sequence_side(lhs) &&
        let Some(rhs_side) = sequence_side(rhs) &&
        (is_reference(lhs) || is_reference(rhs))
        else {
            return Err(RuleNotApplicable);
        }
    );

    let eq_expr = sequence_eq_expr(&lhs_side, &rhs_side);
    let new_expr = if neq {
        Expr::Not(Metadata::new(), Moo::new(eq_expr))
    } else {
        eq_expr
    };
    Ok(Reduction::pure(new_expr))
}

fn sequence_eq_expr(lhs: &SeqSide, rhs: &SeqSide) -> Expr {
    let len_eq = Expr::Eq(
        Metadata::new(),
        Moo::new(lhs.length_expr.clone()),
        Moo::new(rhs.length_expr.clone()),
    );
    let common = lhs.max_size.min(rhs.max_size);
    let mut constraints = vec![len_eq];
    for idx in 1..=common {
        let inactive_guard = Expr::Gt(
            Metadata::new(),
            Moo::new(idx.into()),
            Moo::new(lhs.length_expr.clone()),
        );
        let value_match = Expr::Eq(
            Metadata::new(),
            Moo::new(lhs.slot[(idx - 1) as usize].clone()),
            Moo::new(rhs.slot[(idx - 1) as usize].clone()),
        );
        constraints.push(Expr::Or(
            Metadata::new(),
            Moo::new(matrix_expr![inactive_guard, value_match]),
        ));
    }
    Expr::And(Metadata::new(), Moo::new(into_matrix_expr![constraints]))
}

/// `a substring b`: does `a` occur as a contiguous run inside `b`?
///
/// Both sides have a statically known maximum size, so every candidate start offset and every
/// active position of `a` can be unrolled directly; a candidate offset that would run past `b`'s
/// own maximum size simply never matches.
/// ```plain
/// a substring b
/// ~>
/// or([ and([ idx > |a| \/ a[idx] = b[i + idx] | idx : int(1..aMaxSize) ])
///    | i : int(0..max(aMaxSize, bMaxSize) - 1)
///    ])
/// ```
#[register_rule("ReprGeneral", 9500, [Substring])]
fn sequence_substring(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Substring(_, a, b) = expr &&
        let Some(a_side) = sequence_side(a) &&
        let Some(b_side) = sequence_side(b) &&
        (is_reference(a) || is_reference(b))
        else {
            return Err(RuleNotApplicable);
        }
    );

    Ok(Reduction::pure(sequence_substring_expr(&a_side, &b_side)))
}

fn sequence_substring_expr(a: &SeqSide, b: &SeqSide) -> Expr {
    let max_offset = a.max_size.max(b.max_size) - 1;
    let mut disjuncts = Vec::new();
    for i in 0..=max_offset {
        let mut and_terms = Vec::new();
        for idx in 1..=a.max_size {
            let inactive_guard = Expr::Gt(
                Metadata::new(),
                Moo::new(idx.into()),
                Moo::new(a.length_expr.clone()),
            );
            let offset = i + idx;
            let value_match = if offset >= 1 && offset <= b.max_size {
                Expr::Eq(
                    Metadata::new(),
                    Moo::new(a.slot[(idx - 1) as usize].clone()),
                    Moo::new(b.slot[(offset - 1) as usize].clone()),
                )
            } else {
                Literal::Bool(false).into()
            };
            and_terms.push(Expr::Or(
                Metadata::new(),
                Moo::new(matrix_expr![inactive_guard, value_match]),
            ));
        }
        disjuncts.push(Expr::And(
            Metadata::new(),
            Moo::new(into_matrix_expr![and_terms]),
        ));
    }
    Expr::Or(Metadata::new(), Moo::new(into_matrix_expr![disjuncts]))
}

/// `a subsequence b`: does `a` occur as an order-preserving (not necessarily contiguous) run
/// inside `b`?
///
/// Enumerates every strictly increasing choice of up to `min(aMaxSize, bMaxSize)` positions in
/// `b`, for every possible active length of `a` up to that bound, rather than introducing an
/// auxiliary index-mapping variable (Conjure's general approach). This is only practical for the
/// small `maxSize` values exercised by the imported exhaustive cases; a bigger `bMaxSize` would
/// need the auxiliary-variable encoding instead.
/// ```plain
/// a subsequence b
/// ~>
/// or([ |a| <= k /\ and([ idx > |a| \/ (j[idx] <= |b| /\ a[idx] = b[j[idx]]) | idx : int(1..k) ])
///    | k : int(0..min(aMaxSize, bMaxSize))
///    , j <- strictly increasing k-combinations of int(1..bMaxSize)
///    ])
/// ```
#[register_rule("ReprGeneral", 9500, [Subsequence])]
fn sequence_subsequence(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Subsequence(_, a, b) = expr &&
        let Some(a_side) = sequence_side(a) &&
        let Some(b_side) = sequence_side(b) &&
        (is_reference(a) || is_reference(b))
        else {
            return Err(RuleNotApplicable);
        }
    );

    Ok(Reduction::pure(sequence_subsequence_expr(&a_side, &b_side)))
}

fn sequence_subsequence_expr(a: &SeqSide, b: &SeqSide) -> Expr {
    let k_max = a.max_size.min(b.max_size);
    let mut disjuncts = Vec::new();
    for k in 0..=k_max {
        for combo in (1..=b.max_size).combinations(k as usize) {
            let mut and_terms = vec![Expr::Leq(
                Metadata::new(),
                Moo::new(a.length_expr.clone()),
                Moo::new(k.into()),
            )];
            for (position, j) in combo.into_iter().enumerate() {
                let idx = (position + 1) as i32;
                let inactive_guard = Expr::Gt(
                    Metadata::new(),
                    Moo::new(idx.into()),
                    Moo::new(a.length_expr.clone()),
                );
                let active_guard = Expr::Leq(
                    Metadata::new(),
                    Moo::new(j.into()),
                    Moo::new(b.length_expr.clone()),
                );
                let value_match = Expr::Eq(
                    Metadata::new(),
                    Moo::new(a.slot[position].clone()),
                    Moo::new(b.slot[(j - 1) as usize].clone()),
                );
                let matched = Expr::And(
                    Metadata::new(),
                    Moo::new(matrix_expr![active_guard, value_match]),
                );
                and_terms.push(Expr::Or(
                    Metadata::new(),
                    Moo::new(matrix_expr![inactive_guard, matched]),
                ));
            }
            disjuncts.push(Expr::And(
                Metadata::new(),
                Moo::new(into_matrix_expr![and_terms]),
            ));
        }
    }
    Expr::Or(Metadata::new(), Moo::new(into_matrix_expr![disjuncts]))
}

use super::TuplePacked;
use crate::guard;
use crate::shared::utils::{
    as_cmp_or_lex_op, as_eq_or_neq, collect_cmp_exprs, collect_eq_or_neq, eq_or_neq,
    tuple_expr_entries,
};
use crate::types::tuple::TupleComponents;
use conjure_cp::ast::{
    Atom, Expression as Expr, Literal, Metadata, Moo, Reference, SymbolTable, eval_constant,
};
use conjure_cp::bug_assert;
use conjure_cp::representation::ReprRule;
use conjure_cp::rule_engine::ApplicationError::{self, RuleNotApplicable};
use conjure_cp::rule_engine::{
    ApplicationResult, RuleEffect as Reduction, register_rule, register_rule_set,
};
use conjure_cp::{essence_expr, into_matrix_expr};
use parking_lot::MappedRwLockReadGuard;

fn comparison_reference(expr: &Expr) -> Option<Reference> {
    if let Expr::Atomic(_, Atom::Reference(reference)) = expr {
        return Some(reference.clone());
    }
    let values = expr.unwrap_list()?;
    let [Expr::Atomic(_, Atom::Reference(reference))] = values.as_slice() else {
        return None;
    };
    Some(reference.clone())
}

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
        lp.values.len() == rp.values.len(),
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
        repr.values.len() == rhs_ents.len(),
        "equality on tuples with different shapes!"
    );

    let packed_val = encode_entries(&repr, &rhs_ents)?;
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
        let Some(lhs_re) = comparison_reference(lhs.as_ref())       &&
        let Some(rhs_re) = comparison_reference(rhs.as_ref())       &&
        let Some(lp) = lhs_re.get_repr_as::<TuplePacked>()          &&
        let Some(rp) = rhs_re.get_repr_as::<TuplePacked>()
        else { return Err(RuleNotApplicable) }
    );

    bug_assert!(
        lp.values.len() == rp.values.len(),
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

    let packed_val = encode_entries(&repr, &rhs_ents)?;
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
    bug_assert!(i < repr.values.len(), "tuple indexing is out of bounds");

    let remaining = &indices[1..];
    if remaining.is_empty() {
        Ok(Reduction::pure(unpack_entry(&repr, i)))
    } else if let Some(projected) = project_values(&repr.values[i], remaining) {
        Ok(Reduction::pure(unpack_values(&repr, i, &projected)))
    } else {
        Ok(Reduction::pure(Expr::SafeIndex(
            Metadata::new(),
            unpack_entry(&repr, i).into(),
            Vec::from(remaining),
        )))
    }
}

/// Channeling constraint between TupleComponents and TuplePacked for the same variable.
/// Handles equalities of the form `x#TupleComponents = x#TuplePacked` (or reversed).
/// ```plain
/// x#TupleComponents = x#TuplePacked
/// ~>
/// decoded packed fields = component fields
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

    let fields = atom.elems.iter().enumerate().map(|(index, declaration)| {
        (
            unpack_entry(&packed, index),
            Expr::from(Reference::new(declaration.clone())),
        )
    });
    Ok(Reduction::pure(collect_eq_or_neq(neq, fields)))
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
    lhs.values == rhs.values
}

fn unpack_entries(repr: &PackedState<'_>) -> Vec<Expr> {
    (0..repr.values.len())
        .map(|index| unpack_entry(repr, index))
        .collect()
}

fn unpack_entry(repr: &PackedState<'_>, index: usize) -> Expr {
    unpack_values(repr, index, &repr.values[index])
}

fn unpack_values(repr: &PackedState<'_>, index: usize, values: &[Literal]) -> Expr {
    let packed = repr.packed_expr();
    let place = repr.places[index];
    let radix = repr.radices[index];
    let digit = match (place, radix, index) {
        (_, 1, _) => Expr::from(0),
        (1, _, 0) => packed,
        (_, _, 0) => essence_expr!(&packed / &place),
        (1, radix, _) => essence_expr!(&packed % &radix),
        (_, radix, _) => essence_expr!((&packed / &place) % &radix),
    };
    if let Some(minimum) = contiguous_int_min(values) {
        return match minimum {
            0 => digit,
            minimum => essence_expr!(&digit + &minimum),
        };
    }
    let values = values.iter().cloned().map(Expr::from).collect::<Vec<_>>();
    Expr::SafeIndex(
        Metadata::new(),
        Moo::new(into_matrix_expr!(values)),
        vec![essence_expr!(&digit + 1)],
    )
}

fn project_values(values: &[Literal], indices: &[Expr]) -> Option<Vec<Literal>> {
    values
        .iter()
        .map(|value| {
            indices.iter().try_fold(value.clone(), |value, index| {
                let Literal::Int(index) = eval_constant(index)? else {
                    return None;
                };
                let Literal::AbstractLiteral(conjure_cp::ast::AbstractLiteral::Tuple(fields)) =
                    value
                else {
                    return None;
                };
                usize::try_from(index - 1)
                    .ok()
                    .and_then(|index| fields.get(index).cloned())
            })
        })
        .collect()
}

fn contiguous_int_min(values: &[Literal]) -> Option<i32> {
    let Literal::Int(minimum) = values.first()? else {
        return None;
    };
    values
        .iter()
        .enumerate()
        .all(|(offset, value)| *value == Literal::Int(*minimum + offset as i32))
        .then_some(*minimum)
}

fn encode_entries(repr: &PackedState<'_>, entries: &[Expr]) -> Result<Expr, ApplicationError> {
    let values = entries
        .iter()
        .map(eval_constant)
        .collect::<Option<Vec<_>>>()
        .ok_or(RuleNotApplicable)?;
    repr.encode(&values)
        .map(Expr::from)
        .ok_or(RuleNotApplicable)
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

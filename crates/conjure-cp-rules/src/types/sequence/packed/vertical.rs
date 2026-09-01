use super::SequencePacked;
use crate::guard;
use crate::shared::utils::{as_eq_or_neq, collect_eq_or_neq};
use crate::types::sequence::SequenceExplicit;
use conjure_cp::ast::{Atom, Expression as Expr, Reference, SymbolTable};
use conjure_cp::representation::ReprRule;
use conjure_cp::rule_engine::ApplicationError::RuleNotApplicable;
use conjure_cp::rule_engine::{
    ApplicationResult, RuleEffect as Reduction, register_rule, register_rule_set,
};
use parking_lot::MappedRwLockReadGuard;

// Packed sequences are backend-neutral, so make their lowering rules available for every
// solver family.
register_rule_set!("ReprSequencePacked", ("Base"), |_| true);

/// Channelling constraint between SequenceExplicit and SequencePacked for the same variable.
/// ```plain
/// x#SequenceExplicit = x#SequencePacked
/// ~>
/// decoded packed positions = explicit positions, decoded length = explicit length
/// ```
#[register_rule("ReprSequencePacked", 9700, [Eq, Neq])]
fn sequence_channel_explicit_packed(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;

    guard!(
        let Expr::Atomic(_, Atom::Reference(re_a)) = lhs &&
        let Expr::Atomic(_, Atom::Reference(re_b)) = rhs &&
        let Some((packed, explicit)) = as_channeling_pair(re_a, re_b)
        else {
            return Err(RuleNotApplicable);
        }
    );

    let (_, max) = explicit.size_bounds;
    let positions = (1..=max).map(|i| (packed.slot_expr(i), explicit.slot_expr(i)));
    let fields = std::iter::once((packed.length_expr(), explicit.length_expr())).chain(positions);
    Ok(Reduction::pure(collect_eq_or_neq(neq, fields)))
}

type PackedState<'a> = MappedRwLockReadGuard<'a, <SequencePacked as ReprRule>::DeclLevel>;
type ExplicitState<'a> = MappedRwLockReadGuard<'a, <SequenceExplicit as ReprRule>::DeclLevel>;
fn as_channeling_pair<'a>(
    lhs: &'a Reference,
    rhs: &'a Reference,
) -> Option<(PackedState<'a>, ExplicitState<'a>)> {
    if lhs.ptr != rhs.ptr {
        return None;
    }
    let packed = match (
        lhs.get_repr_as::<SequencePacked>(),
        rhs.get_repr_as::<SequencePacked>(),
    ) {
        (Some(lhs), None) => lhs,
        (None, Some(rhs)) => rhs,
        _ => return None,
    };
    let explicit = match (
        lhs.get_repr_as::<SequenceExplicit>(),
        rhs.get_repr_as::<SequenceExplicit>(),
    ) {
        (Some(lhs), None) => lhs,
        (None, Some(rhs)) => rhs,
        _ => return None,
    };
    Some((packed, explicit))
}

//! Shared lowering of sequence application, `s(i)`.

use conjure_cp::ast::{Expression as Expr, GroundDomain, Metadata, Moo, Range, eval_constant};

/// Builds the value a sequence takes at position `index`, given the values of every position it
/// could have.
///
/// A sequence is a partial function: applying it outside `1..|s|` is undefined. `positions` is
/// indexed unsafely so that the usual bubble machinery discharges positions the sequence does not
/// have at all, and a sequence whose length varies gets a second condition for the positions that
/// exist in the representation but sit past the active length -- without it, those would quietly
/// answer with the padding value.
pub(crate) fn apply_at_position(
    positions: Expr,
    index: Expr,
    size_bounds: (i32, i32),
    length: impl FnOnce() -> Expr,
) -> Expr {
    let slot = Expr::UnsafeIndex(Metadata::new(), Moo::new(positions), vec![index.clone()]);

    let (min_length, max_length) = size_bounds;
    if min_length == max_length {
        // Every position the representation has is an active one, so the index bounds are the
        // whole story and the bubble for those says all there is to say.
        return slot;
    }

    // The allocated matrix is `1..max_length`; the *active* prefix is only `1..min_length` when
    // the length still varies. An index can sit in the matrix and still be past `|s|`.
    if index_in_active_prefix(&index, min_length) {
        return slot;
    }

    Expr::Bubble(
        Metadata::new(),
        Moo::new(slot),
        Moo::new(Expr::Leq(
            Metadata::new(),
            Moo::new(index),
            Moo::new(length()),
        )),
    )
}

/// Whether every value `index` can take lies in `1..=min_length`.
///
/// `min_length == 0` cannot prove anything: the sequence may be empty, so even `1` is undefined.
fn index_in_active_prefix(index: &Expr, min_length: i32) -> bool {
    if min_length <= 0 {
        return false;
    }
    let prefix = GroundDomain::Int(vec![Range::Bounded(1, min_length)]);
    if let Some(lit) = eval_constant(index) {
        return prefix.contains(&lit).unwrap_or(false);
    }
    let Some(index_domain) = index.domain_of().and_then(|domain| domain.resolve().ok()) else {
        return false;
    };
    index_domain
        .intersect(&prefix)
        .is_ok_and(|intersection| intersection == *index_domain.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_positions() -> Expr {
        Expr::from(0)
    }

    #[test]
    fn constant_index_inside_min_length_has_no_length_bubble() {
        let result = apply_at_position(dummy_positions(), 2.into(), (3, 5), || {
            panic!("length should not be needed when the index is inside the active prefix")
        });
        assert!(
            !matches!(result, Expr::Bubble(..)),
            "expected no bubble, got {result}"
        );
    }

    #[test]
    fn constant_index_past_min_length_keeps_the_length_bubble() {
        let length = Expr::from(4);
        let result = apply_at_position(dummy_positions(), 4.into(), (3, 5), || length.clone());
        let Expr::Bubble(_, _, condition) = result else {
            panic!("expected a bubble for an index that may exceed the active length");
        };
        assert!(matches!(condition.as_ref(), Expr::Leq(..)));
    }

    #[test]
    fn min_length_zero_never_skips_the_length_bubble() {
        let length = Expr::from(0);
        let result = apply_at_position(dummy_positions(), 1.into(), (0, 5), || length.clone());
        assert!(
            matches!(result, Expr::Bubble(..)),
            "index 1 is not safe when the sequence may be empty, got {result}"
        );
    }
}

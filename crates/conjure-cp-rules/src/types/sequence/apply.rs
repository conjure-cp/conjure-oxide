//! Shared lowering of sequence application, `s(i)`.

use conjure_cp::ast::{Expression as Expr, Metadata, Moo};

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

mod explicit;
mod occurrence;
mod packed;

pub use explicit::MSetExplicit;
pub use occurrence::MSetOccurrence;
pub use packed::MSetPacked;

use conjure_cp::ast::{MSetAttr, Range};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MSetBounds {
    cardinality: (i32, i32),
    occurrence: (i32, i32),
}

fn mset_bounds(attrs: &MSetAttr<i32>, inner_len: usize) -> Option<MSetBounds> {
    let inner_len = i32::try_from(inner_len).ok()?;
    let min_occurrence = range_lower(&attrs.occurrence).unwrap_or(0);
    let explicit_max_size = range_upper(&attrs.size);
    let max_occurrence = range_upper(&attrs.occurrence).or(explicit_max_size)?;
    if min_occurrence < 0 || max_occurrence < min_occurrence {
        return None;
    }

    let intrinsic_min_size = min_occurrence.checked_mul(inner_len)?;
    let intrinsic_max_size = max_occurrence.checked_mul(inner_len)?;
    let min_size = range_lower(&attrs.size)
        .unwrap_or(0)
        .max(intrinsic_min_size);
    let max_size = explicit_max_size
        .unwrap_or(intrinsic_max_size)
        .min(intrinsic_max_size);
    (min_size <= max_size).then_some(MSetBounds {
        cardinality: (min_size, max_size),
        occurrence: (min_occurrence, max_occurrence),
    })
}

fn range_lower(range: &Range<i32>) -> Option<i32> {
    match range {
        Range::Single(value) | Range::UnboundedR(value) => Some(*value),
        Range::Bounded(value, _) => Some(*value),
        Range::Unbounded | Range::UnboundedL(_) => None,
    }
}

fn range_upper(range: &Range<i32>) -> Option<i32> {
    match range {
        Range::Single(value) | Range::UnboundedL(value) => Some(*value),
        Range::Bounded(_, value) => Some(*value),
        Range::Unbounded | Range::UnboundedR(_) => None,
    }
}

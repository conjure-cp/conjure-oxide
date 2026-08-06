//! Shared helpers used by every partition representation.

use conjure_cp::ast::{PartitionAttr, Range};

/// Narrows a size-shaped range attribute to be bounded above by `max`, treating an absent lower
/// bound as `0` and an absent upper bound as `max`.
pub(crate) fn bound_size(range: &Range<i32>, max: i32) -> Range<i32> {
    let lo = range.low().copied().unwrap_or(0).max(0);
    let hi = range.high().copied().map(|h| h.min(max)).unwrap_or(max);
    Range::new(Some(lo), Some(hi))
}

/// Resolves a partition's `numParts`/`partSize` attributes into concrete, bounded ranges, and
/// reports whether the part size ends up fixed. Shared by every partition representation so they
/// agree on exactly the same notion of "how big can this get" and "is `regular` redundant here".
///
/// If the partition is `regular` and either `numParts` or `partSize` is already pinned to a single
/// value, the other is forced (`partSize = |innerDomain| / numParts`, or vice versa) -- a regular
/// partition's parts are all the same size, so pinning one of (count, size) with a known total
/// forces the other. This is treated exactly as if the user had written the forced value
/// explicitly, which lets `regular`'s own pairwise "all parts equal size" check be skipped
/// entirely wherever it would otherwise be redundant. When the total doesn't divide evenly no
/// regular partition exists; the attributes are left as given so the model comes out UNSAT via the
/// normal size constraints rather than special-casing infeasibility here.
///
/// Both `numParts` and `partSize` are also bounded above by the inner domain's own size
/// (`max_size`), falling back to that bound whenever the attribute itself leaves size unbounded --
/// mirrors Conjure's own `getMaxNumParts`/`getMaxPartSizes` (`Representations/Partition/
/// Occurrence.hs`), which fall back to `domainSizeOf` the same way.
pub(crate) fn resolve_partition_size_attrs(
    attr: &PartitionAttr,
    max_size: i32,
) -> (Range<i32>, Range<i32>, bool) {
    let inferred_part_len = if attr.is_regular {
        match (
            attr.num_parts.low(),
            attr.num_parts.high(),
            attr.part_len.low(),
            attr.part_len.high(),
        ) {
            (Some(n), Some(n2), _, _) if n == n2 && *n != 0 && max_size % n == 0 => {
                Some(Range::Single(max_size / n))
            }
            (_, _, Some(s), Some(s2)) if s == s2 => None,
            _ => None,
        }
    } else {
        None
    };
    let inferred_num_parts = if attr.is_regular {
        match (
            attr.part_len.low(),
            attr.part_len.high(),
            attr.num_parts.low(),
            attr.num_parts.high(),
        ) {
            (Some(s), Some(s2), np_lo, np_hi)
                if s == s2
                    && *s != 0
                    && max_size % s == 0
                    && !(np_lo.is_some() && np_hi.is_some() && np_lo == np_hi) =>
            {
                Some(Range::Single(max_size / s))
            }
            _ => None,
        }
    } else {
        None
    };
    let num_parts = bound_size(
        &inferred_num_parts.unwrap_or_else(|| attr.num_parts.clone()),
        max_size,
    );
    let part_len = bound_size(
        &inferred_part_len.unwrap_or_else(|| attr.part_len.clone()),
        max_size,
    );
    let fixed_part_size = matches!(part_len, Range::Single(_));
    (num_parts, part_len, fixed_part_size)
}

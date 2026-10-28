//! Shared unsigned-code operations, without enumerating sparse domain spans.
use conjure_cp::ast::{DeclarationPtr, Expression, Literal, Metadata, Moo, Reference};
use conjure_cp::into_matrix_expr;

/// Sort and merge intervals so rank counts each allowed value once.
pub(crate) fn canonical_ranges(mut ranges: Vec<(i32, i32)>) -> Vec<(i32, i32)> {
    ranges.retain(|(low, high)| low <= high);
    ranges.sort_unstable();
    let mut out: Vec<(i32, i32)> = Vec::new();
    for (low, high) in ranges {
        if let Some(last) = out.last_mut()
            && i64::from(low) <= i64::from(last.1) + 1
        {
            last.1 = last.1.max(high);
            continue;
        }
        out.push((low, high));
    }
    out
}
/// Count rank values or values in the offset span.
pub(crate) fn unsigned_capacity(ranges: &[(i32, i32)], rank: bool) -> u64 {
    if rank {
        ranges
            .iter()
            .map(|(lo, hi)| (i64::from(*hi) - i64::from(*lo) + 1) as u64)
            .sum()
    } else {
        (i64::from(ranges.last().unwrap().1) - i64::from(ranges[0].0) + 1) as u64
    }
}
/// Smallest unsigned width, retaining one constrained bit for singletons.
pub(crate) fn unsigned_width(maximum: u64) -> usize {
    (64 - maximum.leading_zeros()).max(1) as usize
}
/// Find the index of an allowed value without expanding intervals.
pub(crate) fn rank_of(ranges: &[(i32, i32)], value: i32) -> Option<u64> {
    let mut start = 0;
    for &(low, high) in ranges {
        if (low..=high).contains(&value) {
            return Some(start + (i64::from(value) - i64::from(low)) as u64);
        }
        start += (i64::from(high) - i64::from(low) + 1) as u64;
    }
    None
}
/// Map an index back to its actual domain value.
pub(crate) fn value_at_rank(ranges: &[(i32, i32)], mut rank: u64) -> Option<i32> {
    for &(low, high) in ranges {
        let count = (i64::from(high) - i64::from(low) + 1) as u64;
        if rank < count {
            return i32::try_from(i64::from(low) + rank as i64).ok();
        }
        rank -= count;
    }
    None
}
/// Read Boolean assignments as an unsigned code.
pub(crate) fn read_code(bits: &[Literal]) -> u64 {
    bits.iter().enumerate().fold(0, |code, (i, bit)| {
        let set = match bit {
            Literal::Bool(set) => *set,
            Literal::Int(0) => false,
            Literal::Int(1) => true,
            other => conjure_cp::bug!("expected a Boolean bit, got {other}"),
        };
        code | (u64::from(set) << i)
    })
}
/// Exclude unused codes, including singleton code one, without an overflowing integer bound.
pub(crate) fn unsigned_bound(bits: &[DeclarationPtr], maximum: u64) -> Expression {
    let mut leq: Expression = true.into();
    for (index, decl) in bits.iter().enumerate() {
        let bit: Expression = Reference::new(decl.clone()).into();
        let not = Expression::Not(Metadata::new(), Moo::new(bit));
        let inputs = Moo::new(into_matrix_expr!(vec![not, leq]));
        leq = if (maximum >> index) & 1 == 1 {
            Expression::Or(Metadata::new(), inputs)
        } else {
            Expression::And(Metadata::new(), inputs)
        };
    }
    leq
}

/// Bound the magnitude separately for each sign before decoding into a narrower signed word.
pub(crate) fn signed_magnitude_bounds(
    bits: &[DeclarationPtr],
    sign: Expression,
    bounds: (i32, i32),
) -> Vec<Expression> {
    let not_sign = Expression::Not(Metadata::new(), Moo::new(sign.clone()));
    let negative = if bounds.0 >= 0 {
        not_sign.clone()
    } else {
        Expression::Imply(
            Metadata::new(),
            Moo::new(sign.clone()),
            Moo::new(unsigned_bound(bits, i64::from(bounds.0).unsigned_abs())),
        )
    };
    let positive = if bounds.1 < 0 {
        sign
    } else {
        Expression::Imply(
            Metadata::new(),
            Moo::new(not_sign),
            Moo::new(unsigned_bound(bits, bounds.1 as u64)),
        )
    };
    vec![negative, positive]
}

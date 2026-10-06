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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::int::{IntOffset, IntRank};
    use conjure_cp::representation::{ReprAssignment, ReprDomainLevel, ReprRule};
    use conjure_cp::{domain_int, range};

    #[test]
    fn sign_bounds_exclude_wrapping_codes_and_keep_valid_values() {
        use conjure_cp::ast::{Atom, Domain, Name, eval_constant};
        use uniplate::Uniplate;
        for (low, high) in [
            (-2, 0),
            (-2, 1),
            (-4, 0),
            (-8, -8),
            (-9, -5),
            (0, 2),
            (i32::MIN, i32::MIN),
            (i32::MIN, i32::MAX),
        ] {
            let magnitude_width = unsigned_width(
                i64::from(low)
                    .unsigned_abs()
                    .max(i64::from(high).unsigned_abs()),
            );
            let bits = (0..magnitude_width)
                .map(|index| {
                    DeclarationPtr::new_find(Name::user(&format!("m{index}")), Domain::bool())
                })
                .collect::<Vec<_>>();
            let sign = DeclarationPtr::new_find(Name::user("sign"), Domain::bool());
            let constraints =
                signed_magnitude_bounds(&bits, Reference::new(sign.clone()).into(), (low, high));
            let width = (1..=32)
                .find(|width| {
                    i64::from(low) >= -(1i64 << (width - 1))
                        && i64::from(high) < (1i64 << (width - 1))
                })
                .unwrap();
            let samples = if magnitude_width <= 8 {
                (0..1u64 << magnitude_width).collect::<Vec<_>>()
            } else {
                vec![0, 1, (1u64 << 31) - 1, 1u64 << 31, u32::MAX as u64]
            };
            for magnitude in samples {
                for negative in [false, true] {
                    let mut values = bits
                        .iter()
                        .enumerate()
                        .map(|(index, bit)| (bit.name().clone(), magnitude & (1 << index) != 0))
                        .collect::<std::collections::HashMap<_, _>>();
                    values.insert(sign.name().clone(), negative);
                    let allowed = constraints.iter().all(|constraint| {
                        let concrete = constraint.transform(&|expression| {
                            let replacement =
                                if let Expression::Atomic(_, Atom::Reference(reference)) =
                                    &expression
                                {
                                    values
                                        .get(&*reference.name())
                                        .copied()
                                        .map(Expression::from)
                                } else {
                                    None
                                };
                            replacement.unwrap_or(expression)
                        });
                        eval_constant(&concrete) == Some(Literal::Bool(true))
                    });
                    let value = if negative {
                        -(magnitude as i64)
                    } else {
                        magnitude as i64
                    };
                    if allowed {
                        assert!(
                            (-(1i64 << (width - 1))..(1i64 << (width - 1))).contains(&value),
                            "code {value} wraps in {width} bits for {low}..{high}"
                        );
                    }
                    if (i64::from(low)..=i64::from(high)).contains(&value)
                        && !(negative && magnitude == 0)
                    {
                        assert!(allowed, "valid code {value} rejected for {low}..{high}");
                    }
                }
            }
        }
    }

    #[test]
    fn unsigned_representations_round_trip_and_reject_gaps() {
        let domain = domain_int!(-3, -1, 2);
        let offset = <IntOffset as ReprRule>::DomainLevel::init(domain.clone()).unwrap();
        let rank = <IntRank as ReprRule>::DomainLevel::init(domain).unwrap();
        assert_eq!(offset.bits.len(), 3);
        assert_eq!(rank.bits.len(), 2);
        for value in [-3, -1, 2] {
            assert_eq!(
                offset.down(Literal::Int(value)).unwrap().up(),
                Literal::Int(value)
            );
            assert_eq!(
                rank.down(Literal::Int(value)).unwrap().up(),
                Literal::Int(value)
            );
        }
        for value in [-4, -2, 0, 1, 3] {
            assert!(offset.down(Literal::Int(value)).is_err());
            assert!(rank.down(Literal::Int(value)).is_err());
        }
        assert_eq!(
            canonical_ranges(vec![(3, 5), (-3, -1), (-2, 0), (1, 1)]),
            vec![(-3, 1), (3, 5)]
        );
        let all = [(i32::MIN, i32::MAX)];
        assert_eq!(unsigned_capacity(&all, true), 1u64 << 32);
        assert_eq!(unsigned_width(unsigned_capacity(&all, true) - 1), 32);
        assert_eq!(value_at_rank(&all, (1u64 << 32) - 1), Some(i32::MAX));
    }
}

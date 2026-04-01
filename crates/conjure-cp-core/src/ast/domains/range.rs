use crate::ast::domains::Int;
use num_traits::Num;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
#[path_prefix(conjure_cp::ast)]
pub enum Range<A = Int> {
    Single(A),
    Bounded(A, A),
    UnboundedL(A),
    UnboundedR(A),
    Unbounded,
}

impl<A> Range<A> {
    /// Whether the range is **bounded** on either side. A bounded range may still be infinite.
    /// See also: [Range::is_finite].
    pub fn is_lower_or_upper_bounded(&self) -> bool {
        match &self {
            Range::Single(_)
            | Range::Bounded(_, _)
            | Range::UnboundedL(_)
            | Range::UnboundedR(_) => true,
            Range::Unbounded => false,
        }
    }

    /// Whether the range is **unbounded** on both sides.
    pub fn is_unbounded(&self) -> bool {
        !self.is_lower_or_upper_bounded()
    }

    /// Whether the range is **finite**. See also: [Range::is_lower_or_upper_bounded].
    pub fn is_finite(&self) -> bool {
        match &self {
            Range::Single(_) | Range::Bounded(_, _) => true,
            Range::Unbounded | Range::UnboundedL(_) | Range::UnboundedR(_) => false,
        }
    }
}

impl<A: Ord> Range<A> {
    pub fn contains(&self, val: &A) -> bool {
        match self {
            Range::Single(x) => x == val,
            Range::Bounded(x, y) => x <= val && val <= y,
            Range::UnboundedR(x) => x <= val,
            Range::UnboundedL(x) => val <= x,
            Range::Unbounded => true,
        }
    }

    /// Returns the lower bound of the range, if it has one
    pub fn low(&self) -> Option<&A> {
        match self {
            Range::Single(a) => Some(a),
            Range::Bounded(a, _) => Some(a),
            Range::UnboundedR(a) => Some(a),
            Range::UnboundedL(_) | Range::Unbounded => None,
        }
    }

    /// Returns the upper bound of the range, if it has one
    pub fn high(&self) -> Option<&A> {
        match self {
            Range::Single(a) => Some(a),
            Range::Bounded(_, a) => Some(a),
            Range::UnboundedL(a) => Some(a),
            Range::UnboundedR(_) | Range::Unbounded => None,
        }
    }
}

impl<A: Ord + Clone> Range<A> {
    /// Create a new range with a lower and upper bound
    pub fn new(lo: Option<A>, hi: Option<A>) -> Range<A> {
        match (lo, hi) {
            (None, None) => Range::Unbounded,
            (Some(l), None) => Range::UnboundedR(l),
            (None, Some(r)) => Range::UnboundedL(r),
            (Some(l), Some(r)) => {
                if l == r {
                    Range::Single(l)
                } else {
                    let min = Ord::min(&l, &r).clone();
                    let max = Ord::max(l, r);
                    Range::Bounded(min, max)
                }
            }
        }
    }

    /// Given a slice of ranges, create a single range that spans from the start
    /// of the leftmost range to the end of the rightmost range.
    /// An empty slice is considered equivalent to `Range::unbounded`.
    pub fn spanning(rngs: &[Range<A>]) -> Range<A> {
        if rngs.is_empty() {
            return Range::Unbounded;
        }

        let mut lo = rngs[0].low();
        let mut hi = rngs[0].high();
        for rng in rngs {
            lo = match (lo, rng.low()) {
                (Some(curr), Some(new)) => Some(curr.min(new)),
                _ => None,
            };
            hi = match (hi, rng.high()) {
                (Some(curr), Some(new)) => Some(curr.max(new)),
                _ => None,
            };
        }
        Range::new(lo.cloned(), hi.cloned())
    }
}

impl<A: Num + Ord + Clone> Range<A> {
    pub fn length(&self) -> Option<A> {
        match self {
            Range::Single(_) => Some(A::one()),
            Range::Bounded(i, j) => Some(j.clone() - i.clone() + A::one()),
            Range::UnboundedR(_) | Range::UnboundedL(_) | Range::Unbounded => None,
        }
    }

    /// Returns true if this interval overlaps another one, i.e. at least one
    /// number is part of both `self` and `other`
    /// E.g:
    /// - [0, 2] overlaps [2, 4]
    /// - [1, 3] overlaps [2, 4]
    /// - [4, 6] overlaps [2, 4]
    pub fn overlaps(&self, other: &Range<A>) -> bool {
        self.low()
            .is_none_or(|la| other.high().is_none_or(|rb| la <= rb))
            && self
                .high()
                .is_none_or(|ra| other.low().is_none_or(|lb| ra >= lb))
    }

    /// Returns true if this interval touches another one on the left
    /// E.g: [1, 2] touches_left  [3, 4]
    pub fn touches_left(&self, other: &Range<A>) -> bool {
        self.high().is_some_and(|ra| {
            let ra = ra.clone() + A::one();
            other.low().is_some_and(|lb| ra.eq(lb))
        })
    }

    /// Returns true if this interval touches another one on the right
    /// E.g: [3, 4] touches_right  [1, 2]
    pub fn touches_right(&self, other: &Range<A>) -> bool {
        self.low().is_some_and(|la| {
            let la = la.clone() - A::one();
            other.high().is_some_and(|rb| la.eq(rb))
        })
    }

    /// Returns true if this interval overlaps or touches another one
    /// E.g:
    /// - [1, 3] joins [4, 6]
    /// - [2, 4] joins [4, 6]
    /// - [3, 5] joins [4, 6]
    /// - [6, 8] joins [4, 6]
    /// - [7, 8] joins [4, 6]
    pub fn joins(&self, other: &Range<A>) -> bool {
        self.touches_left(other) || self.overlaps(other) || self.touches_right(other)
    }

    /// Returns true if this interval is strictly before another one
    pub fn is_before(&self, other: &Range<A>) -> bool {
        self.high()
            .is_some_and(|ra| other.low().is_some_and(|lb| ra < &(lb.clone() - A::one())))
    }

    /// Returns true if this interval is strictly after another one
    pub fn is_after(&self, other: &Range<A>) -> bool {
        self.low()
            .is_some_and(|la| other.high().is_some_and(|rb| la > &(rb.clone() + A::one())))
    }

    /// If the two ranges join, return a new range which spans both
    pub fn join(&self, other: &Range<A>) -> Option<Range<A>> {
        if self.joins(other) {
            let lo = Ord::min(self.low(), other.low());
            let hi = Ord::max(self.high(), other.high());
            return Some(Range::new(lo.cloned(), hi.cloned()));
        }
        None
    }

    /// Merge all joining ranges in the list, and return a new vec of disjoint ranges.
    /// E.g:
    /// ```ignore
    /// [(2..3), (4), (..1), (6..8)] -> [(..4), (6..8)]
    /// ```
    ///
    /// # Performance
    /// Currently uses a naive O(n^2) algorithm.
    /// A more optimal approach based on interval trees is planned.
    pub fn squeeze(rngs: &[Range<A>]) -> Vec<Range<A>> {
        let mut ans = Vec::from(rngs);

        if ans.is_empty() {
            return ans;
        }

        loop {
            let mut merged = false;

            // Check every pair of ranges and join them if possible
            'outer: for i in 0..ans.len() {
                for j in (i + 1)..ans.len() {
                    if let Some(joined) = ans[i].join(&ans[j]) {
                        ans[i] = joined;
                        // Safe to delete here because we restart the outer loop immediately
                        ans.remove(j);
                        merged = true;
                        break 'outer;
                    }
                }
            }

            // If no merges occurred, we're done
            if !merged {
                break;
            }
        }

        ans
    }

    /// If this range is bounded, returns a lazy iterator over all values within the range.
    /// Otherwise, returns None.
    pub fn iter(&self) -> Option<RangeIterator<A>> {
        match self {
            Range::Single(val) => Some(RangeIterator::Single(Some(val.clone()))),
            Range::Bounded(start, end) => Some(RangeIterator::Bounded {
                current: start.clone(),
                end: end.clone(),
            }),
            Range::UnboundedL(_) | Range::UnboundedR(_) | Range::Unbounded => None,
        }
    }

    pub fn values(rngs: &[Range<A>]) -> Option<impl Iterator<Item = A>> {
        let itrs = rngs
            .iter()
            .map(Range::iter)
            .collect::<Option<Vec<RangeIterator<A>>>>()?;
        Some(itrs.into_iter().flatten())
    }
}

/// Iterator for Range<A> that yields values lazily
pub enum RangeIterator<A> {
    Single(Option<A>),
    Bounded { current: A, end: A },
}

impl<A: Num + Ord + Clone> Iterator for RangeIterator<A> {
    type Item = A;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            RangeIterator::Single(val) => val.take(),
            RangeIterator::Bounded { current, end } => {
                if current > end {
                    return None;
                }

                let result = current.clone();
                *current = current.clone() + A::one();

                Some(result)
            }
        }
    }
}

impl<A: Display> Display for Range<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Range::Single(i) => write!(f, "{i}"),
            Range::Bounded(i, j) => write!(f, "{i}..{j}"),
            Range::UnboundedR(i) => write!(f, "{i}.."),
            Range::UnboundedL(i) => write!(f, "..{i}"),
            Range::Unbounded => write!(f, ""),
        }
    }
}

#[allow(unused_imports)]
mod test {
    use super::*;
    use crate::range;

    #[test]
    pub fn test_range_macros() {
        assert_eq!(range!(1..3), Range::Bounded(1, 3));
        assert_eq!(range!(1..), Range::UnboundedR(1));
        assert_eq!(range!(..3), Range::UnboundedL(3));
        assert_eq!(range!(1), Range::Single(1));
    }

    #[test]
    pub fn test_range_low() {
        assert_eq!(range!(1..3).low(), Some(&1));
        assert_eq!(range!(1..).low(), Some(&1));
        assert_eq!(range!(1).low(), Some(&1));
        assert_eq!(range!(..3).low(), None);
        assert_eq!(Range::<Int>::Unbounded.low(), None);
    }

    #[test]
    pub fn test_range_high() {
        assert_eq!(range!(1..3).high(), Some(&3));
        assert_eq!(range!(1..).high(), None);
        assert_eq!(range!(1).high(), Some(&1));
        assert_eq!(range!(..3).high(), Some(&3));
        assert_eq!(Range::<Int>::Unbounded.high(), None);
    }

    #[test]
    pub fn test_range_is_finite() {
        assert!(range!(1..3).is_finite());
        assert!(range!(1).is_finite());
        assert!(!range!(1..).is_finite());
        assert!(!range!(..3).is_finite());
        assert!(!Range::<Int>::Unbounded.is_finite());
    }

    #[test]
    pub fn test_range_bounded() {
        assert!(range!(1..3).is_lower_or_upper_bounded());
        assert!(range!(1).is_lower_or_upper_bounded());
        assert!(range!(1..).is_lower_or_upper_bounded());
        assert!(range!(..3).is_lower_or_upper_bounded());
        assert!(!Range::<Int>::Unbounded.is_lower_or_upper_bounded());
    }

    #[test]
    pub fn test_range_length() {
        assert_eq!(range!(1..3).length(), Some(3));
        assert_eq!(range!(1).length(), Some(1));
        assert_eq!(range!(1..).length(), None);
        assert_eq!(range!(..3).length(), None);
        assert_eq!(Range::<Int>::Unbounded.length(), None);
    }

    #[test]
    pub fn test_range_contains_value() {
        assert!(range!(1..3).contains(&2));
        assert!(!range!(1..3).contains(&4));
        assert!(range!(1).contains(&1));
        assert!(!range!(1).contains(&2));
        assert!(Range::Unbounded.contains(&42));
    }

    #[test]
    pub fn test_range_overlaps() {
        assert!(range!(1..3).overlaps(&range!(2..4)));
        assert!(range!(1..3).overlaps(&range!(3..5)));
        assert!(!range!(1..3).overlaps(&range!(4..6)));
        assert!(Range::Unbounded.overlaps(&range!(1..3)));
    }

    #[test]
    pub fn test_range_touches_left() {
        assert!(range!(1..2).touches_left(&range!(3..4)));
        assert!(range!(1..2).touches_left(&range!(3)));
        assert!(range!(-5..-4).touches_left(&range!(-3..2)));
        assert!(!range!(1..2).touches_left(&range!(4..5)));
        assert!(!range!(1..2).touches_left(&range!(2..3)));
        assert!(!range!(3..4).touches_left(&range!(1..2)));
    }

    #[test]
    pub fn test_range_touches_right() {
        assert!(range!(3..4).touches_right(&range!(1..2)));
        assert!(range!(3).touches_right(&range!(1..2)));
        assert!(range!(0..1).touches_right(&range!(-2..-1)));
        assert!(!range!(1..2).touches_right(&range!(3..4)));
        assert!(!range!(2..3).touches_right(&range!(1..2)));
        assert!(!range!(1..2).touches_right(&range!(1..2)));
    }

    #[test]
    pub fn test_range_is_before() {
        assert!(range!(1..2).is_before(&range!(4..5)));
        assert!(range!(1..2).is_before(&range!(4..)));
        assert!(!range!(1..2).is_before(&range!(3..)));
        assert!(!range!(1..2).is_before(&range!(..4)));
        assert!(!range!(1..2).is_before(&range!(2..4)));
        assert!(!range!(3..4).is_before(&range!(1..2)));
        assert!(!range!(1..2).is_before(&Range::Unbounded));
    }

    #[test]
    pub fn test_range_is_after() {
        assert!(range!(5..6).is_after(&range!(1..2)));
        assert!(range!(4..5).is_after(&range!(..2)));
        assert!(!range!(4..5).is_after(&range!(..3)));
        assert!(!range!(2..3).is_after(&range!(1..2)));
        assert!(!range!(1..2).is_after(&range!(3..4)));
        assert!(!range!(1..2).is_after(&Range::Unbounded));
    }

    #[test]
    pub fn test_range_squeeze() {
        let input = vec![range!(2..3), range!(4), range!(..1), range!(6..8)];
        let squeezed = Range::squeeze(&input);
        assert_eq!(squeezed, vec![range!(..4), range!(6..8)]);
    }

    #[test]
    pub fn test_range_spanning() {
        assert_eq!(Range::<Int>::spanning(&[]), Range::Unbounded);
        assert_eq!(Range::spanning(&[range!(1..2), range!(4..5)]), range!(1..5));
        assert_eq!(
            Range::spanning(&[range!(..0), range!(2..4)]),
            Range::UnboundedL(4)
        );
        assert_eq!(
            Range::spanning(&[range!(0), range!(2..3), range!(5..)]),
            Range::UnboundedR(0)
        );
        assert_eq!(
            Range::spanning(&[range!(..0), range!(2..)]),
            Range::Unbounded
        );
    }
}

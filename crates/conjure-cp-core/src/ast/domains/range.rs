use num_traits::{
    Num,
    sign::{Signed, abs},
};
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
#[path_prefix(conjure_cp::ast)]
pub enum Range<A> {
    Single(A),
    Bounded(A, A),
    UnboundedL(A),
    UnboundedR(A),
    Unbounded,
}

impl<A> Range<A> {
    /// Whether the range is **bounded** on either side. A bounded range may still be infinite.
    /// See also: [Range::is_finite].
    pub fn is_bounded(&self) -> bool {
        match &self {
            Range::Single(_)
            | Range::Bounded(_, _)
            | Range::UnboundedL(_)
            | Range::UnboundedR(_) => true,
            Range::Unbounded => false,
        }
    }

    /// Whether the range is **finite**. See also: [Range::is_bounded].
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
            Range::UnboundedR(x) => x >= val,
            Range::UnboundedL(x) => x <= val,
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
                if (l == r) {
                    Range::Single(l)
                } else {
                    let min = Ord::min(&l, &r).clone();
                    let max = Ord::max(l, r);
                    Range::Bounded(min, max)
                }
            }
        }
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

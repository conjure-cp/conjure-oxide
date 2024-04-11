use serde::{Deserialize, Serialize};
// use std::iter::Ste

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Range<A>
where
    A: Ord,
{
    Single(A),
    Bounded(A, A),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Domain {
    BoolDomain,
    IntDomain(Vec<Range<i32>>),
}

impl Domain {
    /// Return a list of all possible i32 values in the domain if it is an IntDomain.
    pub fn values_i32(&self) -> Option<Vec<i32>> {
        match self {
            Domain::IntDomain(ranges) => Some(
                ranges
                    .iter()
                    .flat_map(|r| match r {
                        Range::Single(i) => vec![*i],
                        Range::Bounded(i, j) => (*i..=*j).collect(),
                    })
                    .collect(),
            ),
            _ => None,
        }
    }

    /// Return an unoptimised domain that is the result of applying a binary i32 operation to two domains.
    ///
    /// The given operator may return None if the operation is not defined for the given arguments.
    /// Undefined values will not be included in the resulting domain.
    ///
    /// Returns None if the domains are not valid for i32 operations.
    pub fn apply_i32(&self, op: fn(i32, i32) -> Option<i32>, other: &Domain) -> Option<Domain> {
        if let (Some(vs1), Some(vs2)) = (self.values_i32(), other.values_i32()) {
            // TODO: (flm8) Optimise to use smarter, less brute-force methods
            let mut new_ranges = vec![];
            for (v1, v2) in itertools::iproduct!(vs1, vs2) {
                op(v1, v2).map(|v| new_ranges.push(Range::Single(v)));
            }
            return Some(Domain::IntDomain(new_ranges));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_negative_product() {
        let d1 = Domain::IntDomain(vec![Range::Bounded(-2, 1)]);
        let d2 = Domain::IntDomain(vec![Range::Bounded(-2, 1)]);
        let res = d1.apply_i32(|a, b| Some(a * b), &d2).unwrap();

        if let Domain::IntDomain(ranges) = res {
            assert!(!ranges.contains(&Range::Single(-4)));
            assert!(!ranges.contains(&Range::Single(-3)));
            assert!(ranges.contains(&Range::Single(-2)));
            assert!(ranges.contains(&Range::Single(-1)));
            assert!(ranges.contains(&Range::Single(0)));
            assert!(ranges.contains(&Range::Single(1)));
            assert!(ranges.contains(&Range::Single(2)));
            assert!(!ranges.contains(&Range::Single(3)));
            assert!(ranges.contains(&Range::Single(4)));
        } else {
            panic!();
        }
    }

    #[test]
    fn test_negative_div() {
        let d1 = Domain::IntDomain(vec![Range::Bounded(-2, 1)]);
        let d2 = Domain::IntDomain(vec![Range::Bounded(-2, 1)]);
        let res = d1
            .apply_i32(|a, b| if b != 0 { Some(a / b) } else { None }, &d2)
            .unwrap();

        if let Domain::IntDomain(ranges) = res {
            assert!(!ranges.contains(&Range::Single(-4)));
            assert!(!ranges.contains(&Range::Single(-3)));
            assert!(ranges.contains(&Range::Single(-2)));
            assert!(ranges.contains(&Range::Single(-1)));
            assert!(ranges.contains(&Range::Single(0)));
            assert!(ranges.contains(&Range::Single(1)));
            assert!(ranges.contains(&Range::Single(2)));
            assert!(!ranges.contains(&Range::Single(3)));
            assert!(!ranges.contains(&Range::Single(4)));
        } else {
            panic!();
        }
    }
}

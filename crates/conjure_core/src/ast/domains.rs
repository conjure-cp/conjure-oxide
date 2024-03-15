use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Range<A> {
    Single(A),
    Bounded(A, A),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Domain {
    BoolDomain,
    IntDomain(Vec<Range<i32>>),
}

impl Domain {
    /// Returns the minimum i32 value a variable of the domain can take, if it is an i32 domain.
    pub fn min_i32(&self) -> Option<i32> {
        match self {
            Domain::BoolDomain => Some(0),
            Domain::IntDomain(ranges) => {
                if ranges.is_empty() {
                    return None;
                }
                let mut min = i32::MAX;
                for r in ranges {
                    match r {
                        Range::Single(i) => min = min.min(*i),
                        Range::Bounded(i, _) => min = min.min(*i),
                    }
                }
                Some(min)
            }
        }
    }

    /// Returns the maximum i32 value a variable of the domain can take, if it is an i32 domain.
    pub fn max_i32(&self) -> Option<i32> {
        match self {
            Domain::BoolDomain => Some(1),
            Domain::IntDomain(ranges) => {
                if ranges.is_empty() {
                    return None;
                }
                let mut max = i32::MIN;
                for r in ranges {
                    match r {
                        Range::Single(i) => max = max.max(*i),
                        Range::Bounded(_, i) => max = max.max(*i),
                    }
                }
                Some(max)
            }
        }
    }

    /// Returns the minimum and maximum integer values a variable of the domain can take, if it is an integer domain.
    pub fn min_max_i32(&self) -> Option<(i32, i32)> {
        match self {
            Domain::BoolDomain => Some((0, 1)),
            Domain::IntDomain(ranges) => {
                if ranges.is_empty() {
                    return None;
                }
                let mut min = i32::MAX;
                let mut max = i32::MIN;
                for r in ranges {
                    match r {
                        Range::Single(i) => {
                            min = min.min(*i);
                            max = max.max(*i);
                        }
                        Range::Bounded(i, j) => {
                            min = min.min(*i);
                            max = max.max(*j);
                        }
                    }
                }
                Some((min, max))
            }
        }
    }
}

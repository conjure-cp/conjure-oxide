use std::fmt::Display;

use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::ast::pretty::pretty_vec;

use super::{types::Typeable, Name, ReturnType};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Range<A>
where
    A: Ord,
{
    Single(A),
    Bounded(A, A),

    /// int(i..)
    UnboundedR(A),

    /// int(..i)
    UnboundedL(A),
}

impl<A: Ord + Display> Display for Range<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Range::Single(i) => write!(f, "{i}"),
            Range::Bounded(i, j) => write!(f, "{i}..{j}"),
            Range::UnboundedR(i) => write!(f, "{i}.."),
            Range::UnboundedL(i) => write!(f, "..{i}"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Domain {
    BoolDomain,
    IntDomain(Vec<Range<i32>>),
    DomainReference(Name),
    DomainSet(SetAttr, Box<Domain>),
    /// A n-dimensional matrix with a value domain and n-index domains
    DomainMatrix(Box<Domain>, Vec<Domain>),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SetAttr {
    None,
    Size(i32),
    MinSize(i32),
    MaxSize(i32),
    MinMaxSize(i32, i32),
}
impl Domain {
    /// Return a list of all possible i32 values in the domain if it is an IntDomain and is
    /// bounded.
    pub fn values_i32(&self) -> Option<Vec<i32>> {
        match self {
            Domain::IntDomain(ranges) => Some(
                ranges
                    .iter()
                    .map(|r| match r {
                        Range::Single(i) => Some(vec![*i]),
                        Range::Bounded(i, j) => Some((*i..=*j).collect()),
                        Range::UnboundedR(_) => None,
                        Range::UnboundedL(_) => None,
                    })
                    .while_some()
                    .flatten()
                    .collect_vec(),
            ),
            _ => None,
        }
    }

    /// Return an unoptimised domain that is the result of applying a binary i32 operation to two domains.
    ///
    /// The given operator may return None if the operation is not defined for its arguments.
    /// Undefined values will not be included in the resulting domain.
    ///
    /// Returns None if the domains are not valid for i32 operations.
    pub fn apply_i32(&self, op: fn(i32, i32) -> Option<i32>, other: &Domain) -> Option<Domain> {
        if let (Some(vs1), Some(vs2)) = (self.values_i32(), other.values_i32()) {
            // TODO: (flm8) Optimise to use smarter, less brute-force methods
            let mut new_ranges = vec![];
            for (v1, v2) in itertools::iproduct!(vs1, vs2) {
                if let Some(v) = op(v1, v2) {
                    new_ranges.push(Range::Single(v))
                }
            }
            return Some(Domain::IntDomain(new_ranges));
        }
        None
    }
}

impl Display for Domain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Domain::BoolDomain => {
                write!(f, "bool")
            }
            Domain::IntDomain(vec) => {
                let domain_ranges: String = vec.iter().map(|x| format!("{x}")).join(",");

                if domain_ranges.is_empty() {
                    write!(f, "int")
                } else {
                    write!(f, "int({domain_ranges})")
                }
            }
            Domain::DomainReference(name) => write!(f, "{}", name),
            Domain::DomainSet(_, domain) => {
                write!(f, "set of ({})", domain)
            }
            Domain::DomainMatrix(value_domain, index_domains) => {
                write!(
                    f,
                    "matrix indexed by [{}] of {value_domain}",
                    pretty_vec(&index_domains.iter().collect_vec())
                )
            }
        }
    }
}

impl Typeable for Domain {
    fn return_type(&self) -> Option<ReturnType> {
        todo!()
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

        assert!(matches!(res, Domain::IntDomain(_)));
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
        }
    }

    #[test]
    fn test_negative_div() {
        let d1 = Domain::IntDomain(vec![Range::Bounded(-2, 1)]);
        let d2 = Domain::IntDomain(vec![Range::Bounded(-2, 1)]);
        let res = d1
            .apply_i32(|a, b| if b != 0 { Some(a / b) } else { None }, &d2)
            .unwrap();

        assert!(matches!(res, Domain::IntDomain(_)));
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
        }
    }
}

use std::fmt::Display;

use conjure_core::ast::SymbolTable;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::ast::pretty::pretty_vec;
use uniplate::{derive::Uniplate, Uniplate};

use super::{types::Typeable, AbstractLiteral, Literal, Name, ReturnType};

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

impl<A: Ord> Range<A> {
    pub fn contains(&self, val: &A) -> bool {
        match self {
            Range::Single(x) => x == val,
            Range::Bounded(x, y) => x <= val && val <= y,
            Range::UnboundedR(x) => x <= val,
            Range::UnboundedL(x) => x >= val,
        }
    }
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

#[derive(Default, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Uniplate)]
#[uniplate()]
pub enum Domain {
    #[default]
    BoolDomain,

    /// An integer domain.
    ///
    /// + If multiple ranges are inside the domain, the values in the domain are the union of these
    ///   ranges.
    ///
    /// + If no ranges are given, the int domain is considered unconstrained, and can take any
    ///   integer value.
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
    // Whether the literal is a member of this domain.
    //
    // Returns `None` if this cannot be determined (e.g. `self` is a `DomainReference`).
    pub fn contains(&self, lit: &Literal) -> Option<bool> {
        // not adding a generic wildcard condition for all domains, so that this gives a compile
        // error when a domain is added.
        match (self, lit) {
            (Domain::IntDomain(ranges), Literal::Int(x)) => {
                // unconstrained int domain
                if ranges.is_empty() {
                    return Some(true);
                };

                Some(ranges.iter().any(|range| range.contains(x)))
            }
            (Domain::IntDomain(_), _) => Some(false),
            (Domain::BoolDomain, Literal::Bool(_)) => Some(true),
            (Domain::BoolDomain, _) => Some(false),
            (Domain::DomainReference(_), _) => None,

            (
                Domain::DomainMatrix(elem_domain, index_domains),
                Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, idx_domain)),
            ) => {
                let mut index_domains = index_domains.clone();
                if index_domains
                    .pop()
                    .expect("a matrix should have atleast one index domain")
                    != *idx_domain
                {
                    return Some(false);
                };

                // matrix literals are represented as nested 1d matrices, so the elements of
                // the matrix literal will be the inner dimensions of the matrix.
                let next_elem_domain = if index_domains.is_empty() {
                    elem_domain.as_ref().clone()
                } else {
                    Domain::DomainMatrix(elem_domain.clone(), index_domains)
                };

                for elem in elems {
                    if !next_elem_domain.contains(elem)? {
                        return Some(false);
                    }
                }

                Some(true)
            }
            (Domain::DomainMatrix(_, _), _) => Some(false),
            (Domain::DomainSet(_, _), Literal::AbstractLiteral(AbstractLiteral::Set(_))) => {
                todo!()
            }
            (Domain::DomainSet(_, _), _) => Some(false),
        }
    }

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

    /// Gets all the values inside this domain, as a [`Literal`]. Returns `None` if the domain is not
    /// finite.
    pub fn values(&self) -> Option<Vec<Literal>> {
        match self {
            Domain::BoolDomain => Some(vec![false.into(), true.into()]),
            Domain::IntDomain(_) => self
                .values_i32()
                .map(|xs| xs.iter().map(|x| Literal::Int(*x)).collect_vec()),

            // ~niklasdewally: don't know how to define this for collections, so leaving it for
            // now... However, it definitely can be done, as matrices can be indexed by matrices.
            Domain::DomainSet(_, _) => todo!(),
            Domain::DomainMatrix(_, _) => todo!(),
            Domain::DomainReference(_) => None,
        }
    }

    /// Gets the length of this domain.
    ///
    /// Returns `None` if it is not finite.
    pub fn length(&self) -> Option<usize> {
        self.values().map(|x| x.len())
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

    /// Whether this domain has a finite number of values.
    ///
    /// Returns `None` if this cannot be determined, e.g. if `self` is a domain reference.
    pub fn is_finite(&self) -> Option<bool> {
        for domain in self.universe() {
            if let Domain::IntDomain(ranges) = domain {
                if ranges.is_empty() {
                    return Some(false);
                }

                if ranges
                    .iter()
                    .any(|range| matches!(range, Range::UnboundedL(_) | Range::UnboundedR(_)))
                {
                    return Some(false);
                }
            } else if let Domain::DomainReference(_) = domain {
                return None;
            }
        }
        Some(true)
    }

    /// Resolves this domain to a ground domain, using the symbol table provided to resolve
    /// references.
    ///
    /// A domain is ground iff it is not a domain reference, nor contains any domain references.
    ///
    /// See also: [`SymbolTable::resolve_domain`](crate::ast::SymbolTable::resolve_domain).
    ///
    /// # Panics
    ///
    /// + If a reference domain in `self` does not exist in the given symbol table.
    pub fn resolve(mut self, symbols: &SymbolTable) -> Domain {
        // FIXME: cannot use Uniplate::transform here due to reference lifetime shenanigans...
        // dont see any reason why Uniplate::transform requires a closure that only uses borrows
        // with a 'static lifetime... ~niklasdewally
        // ..
        // Also, still want to make the Uniplate variant which uses FnOnce not Fn with methods that
        // take self instead of &self -- that would come in handy here!

        let mut done_something = true;
        while done_something {
            done_something = false;
            for (domain, ctx) in self.clone().contexts() {
                if let Domain::DomainReference(name) = domain {
                    self = ctx(symbols
                        .resolve_domain(&name)
                        .expect("domain reference should exist in the symbol table")
                        .resolve(symbols));
                    done_something = true;
                }
            }
        }
        self
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

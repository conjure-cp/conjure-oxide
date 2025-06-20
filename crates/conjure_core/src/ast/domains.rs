use std::fmt::Display;

use conjure_core::ast::SymbolTable;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::ast::pretty::pretty_vec;
use uniplate::{derive::Uniplate, Uniplate};

use super::{records::RecordEntry, types::Typeable, AbstractLiteral, Literal, Name, ReturnType};

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

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Uniplate)]
#[uniplate()]
pub enum Domain {
    Bool,

    /// An integer domain.
    ///
    /// + If multiple ranges are inside the domain, the values in the domain are the union of these
    ///   ranges.
    ///
    /// + If no ranges are given, the int domain is considered unconstrained, and can take any
    ///   integer value.
    Int(Vec<Range<i32>>),
    Reference(Name),
    Set(SetAttr, Box<Domain>),
    /// A n-dimensional matrix with a value domain and n-index domains
    Matrix(Box<Domain>, Vec<Domain>),
    // A tuple of n domains (e.g. (int, bool))
    Tuple(Vec<Domain>),

    Record(Vec<RecordEntry>),
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
            (Domain::Int(ranges), Literal::Int(x)) => {
                // unconstrained int domain
                if ranges.is_empty() {
                    return Some(true);
                };

                Some(ranges.iter().any(|range| range.contains(x)))
            }
            (Domain::Int(_), _) => Some(false),
            (Domain::Bool, Literal::Bool(_)) => Some(true),
            (Domain::Bool, _) => Some(false),
            (Domain::Reference(_), _) => None,
            (
                Domain::Matrix(elem_domain, index_domains),
                Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, idx_domain)),
            ) => {
                let mut index_domains = index_domains.clone();
                if index_domains
                    .pop()
                    .expect("a matrix should have atleast one index domain")
                    != **idx_domain
                {
                    return Some(false);
                };

                // matrix literals are represented as nested 1d matrices, so the elements of
                // the matrix literal will be the inner dimensions of the matrix.
                let next_elem_domain = if index_domains.is_empty() {
                    elem_domain.as_ref().clone()
                } else {
                    Domain::Matrix(elem_domain.clone(), index_domains)
                };

                for elem in elems {
                    if !next_elem_domain.contains(elem)? {
                        return Some(false);
                    }
                }

                Some(true)
            }
            (
                Domain::Tuple(elem_domains),
                Literal::AbstractLiteral(AbstractLiteral::Tuple(literal_elems)),
            ) => {
                // for every element in the tuple literal, check if it is in the corresponding domain
                for (elem_domain, elem) in itertools::izip!(elem_domains, literal_elems) {
                    if !elem_domain.contains(elem)? {
                        return Some(false);
                    }
                }

                Some(true)
            }
            (
                Domain::Set(_, domain),
                Literal::AbstractLiteral(AbstractLiteral::Set(literal_elems)),
            ) => {
                for elem in literal_elems {
                    if !domain.contains(elem)? {
                        return Some(false);
                    }
                }
                Some(true)
            }
            (
                Domain::Record(entries),
                Literal::AbstractLiteral(AbstractLiteral::Record(lit_entries)),
            ) => {
                for (entry, lit_entry) in itertools::izip!(entries, lit_entries) {
                    if entry.name != lit_entry.name || !(entry.domain.contains(&lit_entry.value)?) {
                        return Some(false);
                    }
                }
                Some(true)
            }

            (Domain::Record(_), _) => Some(false),

            (Domain::Matrix(_, _), _) => Some(false),

            (Domain::Set(_, _), _) => Some(false),

            (Domain::Tuple(_), _) => Some(false),
        }
    }

    /// Return a list of all possible i32 values in the domain if it is an IntDomain and is
    /// bounded.
    pub fn values_i32(&self) -> Option<Vec<i32>> {
        match self {
            Domain::Int(ranges) => Some(
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

    // turns vector of integers into a domain
    // TODO: can be done more compactly in terms of the domain we produce. e.g. instead of int(1,2,3,4,5,8,9,10) produce int(1..5, 8..10)
    // needs to be tested with domain functions intersect() and uninon() once comprehension rules are written.
    pub fn make_int_domain_from_values_i32(&self, vector: &[i32]) -> Option<Domain> {
        let mut new_ranges = vec![];
        for values in vector.iter() {
            new_ranges.push(Range::Single(*values));
        }
        Some(Domain::Int(new_ranges))
    }

    /// Gets all the values inside this domain, as a [`Literal`]. Returns `None` if the domain is not
    /// finite.
    pub fn values(&self) -> Option<Vec<Literal>> {
        match self {
            Domain::Bool => Some(vec![false.into(), true.into()]),
            Domain::Int(_) => self
                .values_i32()
                .map(|xs| xs.iter().map(|x| Literal::Int(*x)).collect_vec()),

            // ~niklasdewally: don't know how to define this for collections, so leaving it for
            // now... However, it definitely can be done, as matrices can be indexed by matrices.
            Domain::Set(_, _) => todo!(),
            Domain::Matrix(_, _) => todo!(),
            Domain::Reference(_) => None,
            Domain::Tuple(_) => todo!(), // TODO: Can this be done?
            Domain::Record(_) => todo!(),
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
            return Some(Domain::Int(new_ranges));
        }
        None
    }

    /// Whether this domain has a finite number of values.
    ///
    /// Returns `None` if this cannot be determined, e.g. if `self` is a domain reference.
    pub fn is_finite(&self) -> Option<bool> {
        for domain in self.universe() {
            if let Domain::Int(ranges) = domain {
                if ranges.is_empty() {
                    return Some(false);
                }

                if ranges
                    .iter()
                    .any(|range| matches!(range, Range::UnboundedL(_) | Range::UnboundedR(_)))
                {
                    return Some(false);
                }
            } else if let Domain::Reference(_) = domain {
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
                if let Domain::Reference(name) = domain {
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

    // simplified domain intersection function. defined for integer domains of sets
    // TODO: does not consider unbounded domains yet
    // needs to be tested once comprehension rules are written
    pub fn intersect(&self, other: &Domain) -> Option<Domain> {
        match (self, other) {
            (Domain::Set(_, x), Domain::Set(_, y)) => {
                Some(Domain::Set(SetAttr::None, Box::new((*x).intersect(y)?)))
            }
            (Domain::Int(_), Domain::Int(_)) => {
                let mut v: Vec<i32> = vec![];
                if self.is_finite()? && other.is_finite()? {
                    if let (Some(v1), Some(v2)) = (self.values_i32(), other.values_i32()) {
                        for value1 in v1.iter() {
                            if v2.contains(value1) && !v.contains(value1) {
                                v.push(*value1)
                            }
                        }
                    }
                    self.make_int_domain_from_values_i32(&v)
                } else {
                    println!("Unbounded domain");
                    None
                }
            }
            _ => None,
        }
    }

    // simplified domain union function. defined for integer domains of sets
    // TODO: does not consider unbounded domains yet
    // needs to be tested once comprehension rules are written
    pub fn union(&self, other: &Domain) -> Option<Domain> {
        match (self, other) {
            (Domain::Set(_, x), Domain::Set(_, y)) => {
                Some(Domain::Set(SetAttr::None, Box::new((*x).union(y)?)))
            }
            (Domain::Int(_), Domain::Int(_)) => {
                let mut v: Vec<i32> = vec![];
                if self.is_finite()? && other.is_finite()? {
                    if let (Some(v1), Some(v2)) = (self.values_i32(), other.values_i32()) {
                        for value1 in v1.iter() {
                            v.push(*value1);
                        }
                        for value2 in v2.iter() {
                            if !v.contains(value2) {
                                v.push(*value2);
                            }
                        }
                    }
                    self.make_int_domain_from_values_i32(&v)
                } else {
                    println!("Unbounded Domain");
                    None
                }
            }
            _ => None,
        }
    }
}

impl Display for Domain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Domain::Bool => {
                write!(f, "bool")
            }
            Domain::Int(vec) => {
                let domain_ranges: String = vec.iter().map(|x| format!("{x}")).join(",");

                if domain_ranges.is_empty() {
                    write!(f, "int")
                } else {
                    write!(f, "int({domain_ranges})")
                }
            }
            Domain::Reference(name) => write!(f, "{}", name),
            Domain::Set(_, domain) => {
                write!(f, "set of ({})", domain)
            }
            Domain::Matrix(value_domain, index_domains) => {
                write!(
                    f,
                    "matrix indexed by [{}] of {value_domain}",
                    pretty_vec(&index_domains.iter().collect_vec())
                )
            }
            Domain::Tuple(domains) => {
                write!(
                    f,
                    "tuple of ({})",
                    pretty_vec(&domains.iter().collect_vec())
                )
            }
            Domain::Record(entries) => {
                write!(
                    f,
                    "record of ({})",
                    pretty_vec(
                        &entries
                            .iter()
                            .map(|entry| format!("{}: {}", entry.name, entry.domain))
                            .collect_vec()
                    )
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
        let d1 = Domain::Int(vec![Range::Bounded(-2, 1)]);
        let d2 = Domain::Int(vec![Range::Bounded(-2, 1)]);
        let res = d1.apply_i32(|a, b| Some(a * b), &d2).unwrap();

        assert!(matches!(res, Domain::Int(_)));
        if let Domain::Int(ranges) = res {
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
        let d1 = Domain::Int(vec![Range::Bounded(-2, 1)]);
        let d2 = Domain::Int(vec![Range::Bounded(-2, 1)]);
        let res = d1
            .apply_i32(|a, b| if b != 0 { Some(a / b) } else { None }, &d2)
            .unwrap();

        assert!(matches!(res, Domain::Int(_)));
        if let Domain::Int(ranges) = res {
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

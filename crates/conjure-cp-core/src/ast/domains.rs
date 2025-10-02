#![warn(clippy::missing_errors_doc)]

use conjure_cp_core::ast::SymbolTable;
use itertools::{Itertools, izip};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt::Display};
use thiserror::Error;

use crate::{ast::pretty::pretty_vec, domain_int, range};
use polyquine::Quine;
use uniplate::Uniplate;

use super::{AbstractLiteral, Literal, Name, ReturnType, records::RecordEntry, types::Typeable};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
#[path_prefix(conjure_cp::ast)]
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
            Range::UnboundedR(x) => x >= val,
            Range::UnboundedL(x) => x <= val,
        }
    }

    /// Returns the lower bound of the range, if it has one
    pub fn lower_bound(&self) -> Option<&A> {
        match self {
            Range::Single(a) => Some(a),
            Range::Bounded(a, _) => Some(a),
            Range::UnboundedR(a) => Some(a),
            Range::UnboundedL(_) => None,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Uniplate, Hash, Quine)]
#[uniplate()]
#[path_prefix(conjure_cp::ast)]
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

    /// An empty domain of the given type.
    Empty(ReturnType),
    Reference(Name),
    Set(SetAttr, Box<Domain>),
    /// A n-dimensional matrix with a value domain and n-index domains
    Matrix(Box<Domain>, Vec<Domain>),
    // A tuple of n domains (e.g. (int, bool))
    Tuple(Vec<Domain>),

    Record(Vec<RecordEntry>),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
pub enum SetAttr {
    None,
    Size(i32),
    MinSize(i32),
    MaxSize(i32),
    MinMaxSize(i32, i32),
}
impl Domain {
    /// Returns true if `lit` is a member of the domain.
    ///
    /// # Errors
    ///
    /// - [`DomainOpError::InputContainsReference`] if the input domain is a reference or contains
    ///   a reference, meaning that its members cannot be determined.
    pub fn contains(&self, lit: &Literal) -> Result<bool, DomainOpError> {
        // not adding a generic wildcard condition for all domains, so that this gives a compile
        // error when a domain is added.
        match (self, lit) {
            (Domain::Empty(_), _) => Ok(false),
            (Domain::Int(ranges), Literal::Int(x)) => {
                // unconstrained int domain
                if ranges.is_empty() {
                    return Ok(true);
                };

                Ok(ranges.iter().any(|range| range.contains(x)))
            }
            (Domain::Int(_), _) => Ok(false),
            (Domain::Bool, Literal::Bool(_)) => Ok(true),
            (Domain::Bool, _) => Ok(false),
            (Domain::Reference(_), _) => Err(DomainOpError::InputContainsReference),
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
                    return Ok(false);
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
                        return Ok(false);
                    }
                }

                Ok(true)
            }
            (
                Domain::Tuple(elem_domains),
                Literal::AbstractLiteral(AbstractLiteral::Tuple(literal_elems)),
            ) => {
                // for every element in the tuple literal, check if it is in the corresponding domain
                for (elem_domain, elem) in itertools::izip!(elem_domains, literal_elems) {
                    if !elem_domain.contains(elem)? {
                        return Ok(false);
                    }
                }

                Ok(true)
            }
            (
                Domain::Set(_, domain),
                Literal::AbstractLiteral(AbstractLiteral::Set(literal_elems)),
            ) => {
                for elem in literal_elems {
                    if !domain.contains(elem)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (
                Domain::Record(entries),
                Literal::AbstractLiteral(AbstractLiteral::Record(lit_entries)),
            ) => {
                for (entry, lit_entry) in itertools::izip!(entries, lit_entries) {
                    if entry.name != lit_entry.name || !(entry.domain.contains(&lit_entry.value)?) {
                        return Ok(false);
                    }
                }
                Ok(true)
            }

            (Domain::Record(_), _) => Ok(false),

            (Domain::Matrix(_, _), _) => Ok(false),

            (Domain::Set(_, _), _) => Ok(false),

            (Domain::Tuple(_), _) => Ok(false),
        }
    }

    /// Returns a list of all possible values in the domain.
    ///
    /// # Errors
    ///
    /// - [`DomainOpError::InputNotInteger`] if the domain is not an integer domain.
    /// - [`DomainOpError::InputUnbounded`] if the domain is unbounded.
    pub fn values_i32(&self) -> Result<Vec<i32>, DomainOpError> {
        if let Domain::Empty(ReturnType::Int) = self {
            return Ok(vec![]);
        }
        let Domain::Int(ranges) = self else {
            return Err(DomainOpError::InputNotInteger(self.return_type().unwrap()));
        };

        if ranges.is_empty() {
            return Err(DomainOpError::InputUnbounded);
        }

        let mut values = vec![];
        for range in ranges {
            match range {
                Range::Single(i) => {
                    values.push(*i);
                }
                Range::Bounded(i, j) => {
                    values.extend(*i..=*j);
                }
                Range::UnboundedR(_) | Range::UnboundedL(_) => {
                    return Err(DomainOpError::InputUnbounded);
                }
            }
        }

        Ok(values)
    }

    /// Creates an [`Domain::Int`] containing the given integers.
    ///
    /// [`Domain::from_set_i32`] should be used instead where possible, as it is cheaper (it does
    /// not need to sort its input).
    ///
    /// # Examples
    ///
    /// ```
    /// use conjure_cp_core::ast::{Domain,Range};
    /// use conjure_cp_core::{domain_int,range};
    ///
    /// let elements = vec![1,2,3,4,5];
    ///
    /// let domain = Domain::from_slice_i32(&elements);
    ///
    /// assert_eq!(domain,domain_int!(1..5));
    /// ```
    ///
    /// ```
    /// use conjure_cp_core::ast::{Domain,Range};
    /// use conjure_cp_core::{domain_int,range};
    ///
    /// let elements = vec![1,2,4,5,7,8,9,10];
    ///
    /// let domain = Domain::from_slice_i32(&elements);
    ///
    ///
    /// assert_eq!(domain,domain_int!(1..2,4..5,7..10));
    /// ```
    ///
    /// ```
    /// use conjure_cp_core::ast::{Domain,Range,ReturnType};
    ///
    /// let elements = vec![];
    ///
    /// let domain = Domain::from_slice_i32(&elements);
    ///
    /// assert!(matches!(domain,Domain::Empty(ReturnType::Int)))
    /// ```
    pub fn from_slice_i32(elements: &[i32]) -> Domain {
        if elements.is_empty() {
            return Domain::Empty(ReturnType::Int);
        }

        let set = BTreeSet::from_iter(elements.iter().copied());

        Domain::from_set_i32(&set)
    }

    /// Creates an [`Domain::Int`] containing the given integers.
    ///
    /// # Examples
    ///
    /// ```
    /// use conjure_cp_core::ast::{Domain,Range};
    /// use conjure_cp_core::{domain_int,range};
    /// use std::collections::BTreeSet;
    ///
    /// let elements = BTreeSet::from([1,2,3,4,5]);
    ///
    /// let domain = Domain::from_set_i32(&elements);
    ///
    /// assert_eq!(domain,domain_int!(1..5));
    /// ```
    ///
    /// ```
    /// use conjure_cp_core::ast::{Domain,Range};
    /// use conjure_cp_core::{domain_int,range};
    /// use std::collections::BTreeSet;
    ///
    /// let elements = BTreeSet::from([1,2,4,5,7,8,9,10]);
    ///
    /// let domain = Domain::from_set_i32(&elements);
    ///
    /// assert_eq!(domain,domain_int!(1..2,4..5,7..10));
    /// ```
    ///
    /// ```
    /// use conjure_cp_core::ast::{Domain,Range,ReturnType};
    /// use std::collections::BTreeSet;
    ///
    /// let elements = BTreeSet::from([]);
    ///
    /// let domain = Domain::from_set_i32(&elements);
    ///
    /// assert!(matches!(domain,Domain::Empty(ReturnType::Int)))
    /// ```
    pub fn from_set_i32(elements: &BTreeSet<i32>) -> Domain {
        if elements.is_empty() {
            return Domain::Empty(ReturnType::Int);
        }
        if elements.len() == 1 {
            return domain_int!(*elements.first().unwrap());
        }

        let mut elems_iter = elements.iter().copied();

        let mut ranges: Vec<Range<i32>> = vec![];

        // Loop over the elements in ascending order, turning all sequential runs of
        // numbers into ranges.

        // the bounds of the current run of numbers.
        let mut lower = elems_iter
            .next()
            .expect("if we get here, elements should have => 2 elements");
        let mut upper = lower;

        for current in elems_iter {
            // As elements is a BTreeSet, current is always strictly larger than lower.

            if current == upper + 1 {
                // current is part of the current run - we now have the run lower..current
                //
                upper = current;
            } else {
                // the run lower..upper has ended.
                //
                // Add the run lower..upper to the domain, and start a new run.

                if lower == upper {
                    ranges.push(range!(lower));
                } else {
                    ranges.push(range!(lower..upper));
                }

                lower = current;
                upper = current;
            }
        }

        // add the final run to the domain
        if lower == upper {
            ranges.push(range!(lower));
        } else {
            ranges.push(range!(lower..upper));
        }

        Domain::Int(ranges)
    }

    /// For a vector of literals, creates a domain that contains all the elements.
    ///
    /// The literals must all be of the same type.
    ///
    /// For abstract literals, this method merges the element domains of the literals, but not the
    /// index domains. Thus, for fixed-sized abstract literals (matrices, tuples, records, etc.),
    /// all literals in the vector must also have the same size / index domain:
    ///
    /// + Matrices: all literals must have the same index domain.
    /// + Tuples: all literals must have the same number of elements.
    /// + Records: all literals must have the same fields.
    ///
    /// # Errors
    ///
    /// - [DomainOpError::InputWrongType] if the input literals are of a different type to
    ///   each-other, as described above.
    ///
    /// # Examples
    ///
    /// ```
    /// use conjure_cp_core::ast::{Domain,Range,Literal,ReturnType};
    ///
    /// let domain = Domain::from_literal_vec(vec![]);
    /// assert_eq!(domain,Ok(Domain::Empty(ReturnType::Unknown)));
    /// ```
    ///
    /// ```
    /// use conjure_cp_core::ast::{Domain,Range,Literal, AbstractLiteral};
    /// use conjure_cp_core::{domain_int, range, matrix};
    ///
    /// // `[1,2;int(2..3)], [4,5; int(2..3)]` has domain
    /// // `matrix indexed by [int(2..3)] of int(1..2,4..5)`
    ///
    /// let matrix_1 = Literal::AbstractLiteral(matrix![Literal::Int(1),Literal::Int(2);domain_int!(2..3)]);
    /// let matrix_2 = Literal::AbstractLiteral(matrix![Literal::Int(4),Literal::Int(5);domain_int!(2..3)]);
    ///
    /// let domain = Domain::from_literal_vec(vec![matrix_1,matrix_2]);
    ///
    /// let expected_domain = Ok(Domain::Matrix(
    ///     Box::new(domain_int!(1..2,4..5)),vec![domain_int!(2..3)]));
    ///
    /// assert_eq!(domain,expected_domain);
    /// ```
    ///
    /// ```
    /// use conjure_cp_core::ast::{Domain,Range,Literal, AbstractLiteral,DomainOpError};
    /// use conjure_cp_core::{domain_int, range, matrix};
    ///
    /// // `[1,2;int(2..3)], [4,5; int(1..2)]` cannot be combined
    /// // `matrix indexed by [int(2..3)] of int(1..2,4..5)`
    ///
    /// let matrix_1 = Literal::AbstractLiteral(matrix![Literal::Int(1),Literal::Int(2);domain_int!(2..3)]);
    /// let matrix_2 = Literal::AbstractLiteral(matrix![Literal::Int(4),Literal::Int(5);domain_int!(1..2)]);
    ///
    /// let domain = Domain::from_literal_vec(vec![matrix_1,matrix_2]);
    ///
    /// assert_eq!(domain,Err(DomainOpError::InputWrongType));
    /// ```
    ///
    /// ```
    /// use conjure_cp_core::ast::{Domain,Range,Literal, AbstractLiteral};
    /// use conjure_cp_core::{domain_int,range, matrix};
    ///
    /// // `[[1,2; int(1..2)];int(2)], [[4,5; int(1..2)]; int(2)]` has domain
    /// // `matrix indexed by [int(2),int(1..2)] of int(1..2,4..5)`
    ///
    ///
    /// let matrix_1 = Literal::AbstractLiteral(matrix![Literal::AbstractLiteral(matrix![Literal::Int(1),Literal::Int(2);domain_int!(1..2)]); domain_int!(2)]);
    /// let matrix_2 = Literal::AbstractLiteral(matrix![Literal::AbstractLiteral(matrix![Literal::Int(4),Literal::Int(5);domain_int!(1..2)]); domain_int!(2)]);
    ///
    /// let domain = Domain::from_literal_vec(vec![matrix_1,matrix_2]);
    ///
    /// let expected_domain = Ok(Domain::Matrix(
    ///     Box::new(domain_int!(1..2,4..5)),
    ///     vec![domain_int!(2),domain_int!(1..2)]));
    ///
    /// assert_eq!(domain,expected_domain);
    /// ```
    ///
    ///
    pub fn from_literal_vec(literals: Vec<Literal>) -> Result<Domain, DomainOpError> {
        // TODO: use proptest to test this better?

        if literals.is_empty() {
            return Ok(Domain::Empty(ReturnType::Unknown));
        }

        let first_literal = literals.first().unwrap();

        match first_literal {
            Literal::Int(_) => {
                // check all literals are ints, then pass this to Domain::from_set_i32.
                let mut ints = BTreeSet::new();
                for lit in literals {
                    let Literal::Int(i) = lit else {
                        return Err(DomainOpError::InputWrongType);
                    };

                    ints.insert(i);
                }

                Ok(Domain::from_set_i32(&ints))
            }
            Literal::Bool(_) => {
                // check all literals are bools
                if literals.iter().any(|x| !matches!(x, Literal::Bool(_))) {
                    Err(DomainOpError::InputWrongType)
                } else {
                    Ok(Domain::Bool)
                }
            }
            Literal::AbstractLiteral(AbstractLiteral::Set(_)) => {
                let mut all_elems = vec![];

                for lit in literals {
                    let Literal::AbstractLiteral(AbstractLiteral::Set(elems)) = lit else {
                        return Err(DomainOpError::InputWrongType);
                    };

                    all_elems.extend(elems);
                }
                let elem_domain = Domain::from_literal_vec(all_elems)?;

                Ok(Domain::Set(SetAttr::None, Box::new(elem_domain)))
            }

            l @ Literal::AbstractLiteral(AbstractLiteral::Matrix(_, _)) => {
                let mut first_index_domain = vec![];
                // flatten index domains of n-d matrix into list
                let mut l = l.clone();
                while let Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, idx)) = l {
                    assert!(
                        !matches!(idx.as_ref(), Domain::Matrix(_, _)),
                        "n-dimensional matrix literals should be represented as a matrix inside a matrix"
                    );
                    first_index_domain.push(idx.as_ref().clone());
                    l = elems[0].clone();
                }

                let mut all_elems: Vec<Literal> = vec![];

                // check types and index domains
                for lit in &literals {
                    let Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, idx)) = lit else {
                        return Err(DomainOpError::InputContainsReference);
                    };

                    all_elems.extend(elems.clone());

                    let mut index_domain = vec![idx.as_ref().clone()];
                    let mut l = elems[0].clone();
                    while let Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, idx)) = l {
                        assert!(
                            !matches!(idx.as_ref(), Domain::Matrix(_, _)),
                            "n-dimensional matrix literals should be represented as a matrix inside a matrix"
                        );
                        index_domain.push(idx.as_ref().clone());
                        l = elems[0].clone();
                    }

                    if index_domain != first_index_domain {
                        return Err(DomainOpError::InputWrongType);
                    }
                }

                // extract all the terminal elements (those that are not nested matrix literals) from the matrix literal.
                let mut terminal_elements: Vec<Literal> = vec![];
                while let Some(elem) = all_elems.pop() {
                    if let Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, _)) = elem {
                        all_elems.extend(elems);
                    } else {
                        terminal_elements.push(elem);
                    }
                }

                let element_domain = Domain::from_literal_vec(terminal_elements)?;

                Ok(Domain::Matrix(Box::new(element_domain), first_index_domain))
            }

            Literal::AbstractLiteral(AbstractLiteral::Tuple(first_elems)) => {
                let n_fields = first_elems.len();

                // for each field, calculate the element domain and add it to this list
                let mut elem_domains = vec![];

                for i in 0..n_fields {
                    let mut all_elems = vec![];
                    for lit in &literals {
                        let Literal::AbstractLiteral(AbstractLiteral::Tuple(elems)) = lit else {
                            return Err(DomainOpError::InputContainsReference);
                        };

                        if elems.len() != n_fields {
                            return Err(DomainOpError::InputContainsReference);
                        }

                        all_elems.push(elems[i].clone());
                    }

                    elem_domains.push(Domain::from_literal_vec(all_elems)?);
                }

                Ok(Domain::Tuple(elem_domains))
            }

            Literal::AbstractLiteral(AbstractLiteral::Record(first_elems)) => {
                let n_fields = first_elems.len();
                let field_names = first_elems.iter().map(|x| x.name.clone()).collect_vec();

                // for each field, calculate the element domain and add it to this list
                let mut elem_domains = vec![];

                for i in 0..n_fields {
                    let mut all_elems = vec![];
                    for lit in &literals {
                        let Literal::AbstractLiteral(AbstractLiteral::Record(elems)) = lit else {
                            return Err(DomainOpError::InputContainsReference);
                        };

                        if elems.len() != n_fields {
                            return Err(DomainOpError::InputContainsReference);
                        }

                        let elem = elems[i].clone();
                        if elem.name != field_names[i] {
                            return Err(DomainOpError::InputContainsReference);
                        }

                        all_elems.push(elem.value);
                    }

                    elem_domains.push(Domain::from_literal_vec(all_elems)?);
                }

                Ok(Domain::Record(
                    izip!(field_names, elem_domains)
                        .map(|(name, domain)| RecordEntry { name, domain })
                        .collect(),
                ))
            }
        }
    }

    /// Gets all the [`Literal`] values inside this domain.
    ///
    /// # Errors
    ///
    /// - [`DomainOpError::InputNotInteger`] if the domain is not an integer domain.
    /// - [`DomainOpError::InputContainsReference`] if the domain is a reference or contains a
    ///   reference, meaning that its values cannot be determined.
    pub fn values(&self) -> Result<Vec<Literal>, DomainOpError> {
        match self {
            Domain::Empty(_) => Ok(vec![]),
            Domain::Bool => Ok(vec![false.into(), true.into()]),
            Domain::Int(_) => self
                .values_i32()
                .map(|xs| xs.iter().map(|x| Literal::Int(*x)).collect_vec()),

            // ~niklasdewally: don't know how to define this for collections, so leaving it for
            // now... However, it definitely can be done, as matrices can be indexed by matrices.
            Domain::Set(_, _) => todo!(),
            Domain::Matrix(_, _) => todo!(),
            Domain::Reference(_) => Err(DomainOpError::InputContainsReference),
            Domain::Tuple(_) => todo!(), // TODO: Can this be done?
            Domain::Record(_) => todo!(),
        }
    }

    /// Gets the length of this domain.
    ///
    /// # Errors
    ///
    /// - [`DomainOpError::InputUnbounded`] if the input domain is of infinite size.
    /// - [`DomainOpError::InputContainsReference`] if the input domain is or contains a
    ///   domain reference, meaning that its size cannot be determined.
    pub fn length(&self) -> Result<usize, DomainOpError> {
        self.values().map(|x| x.len())
    }

    /// Returns the domain that is the result of applying a binary operation to two integer domains.
    ///
    /// The given operator may return `None` if the operation is not defined for its arguments.
    /// Undefined values will not be included in the resulting domain.
    ///
    /// # Errors
    ///
    /// - [`DomainOpError::InputUnbounded`] if either of the input domains are unbounded.
    /// - [`DomainOpError::InputNotInteger`] if either of the input domains are not integers.
    pub fn apply_i32(
        &self,
        op: fn(i32, i32) -> Option<i32>,
        other: &Domain,
    ) -> Result<Domain, DomainOpError> {
        let vs1 = self.values_i32()?;
        let vs2 = other.values_i32()?;

        let mut set = BTreeSet::new();
        for (v1, v2) in itertools::iproduct!(vs1, vs2) {
            if let Some(v) = op(v1, v2) {
                set.insert(v);
            }
        }

        Ok(Domain::from_set_i32(&set))
    }
    /// Returns true if the domain is finite.
    ///
    /// # Errors
    ///
    /// - [`DomainOpError::InputContainsReference`] if the input domain is or contains a
    ///   domain reference, meaning that its size cannot be determined.
    pub fn is_finite(&self) -> Result<bool, DomainOpError> {
        for domain in self.universe() {
            if let Domain::Int(ranges) = domain {
                if ranges.is_empty() {
                    return Ok(false);
                }

                if ranges
                    .iter()
                    .any(|range| matches!(range, Range::UnboundedL(_) | Range::UnboundedR(_)))
                {
                    return Ok(false);
                }
            } else if let Domain::Reference(_) = domain {
                return Err(DomainOpError::InputContainsReference);
            }
        }
        Ok(true)
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

    /// Calculates the intersection of two domains.
    ///
    /// # Errors
    ///
    ///  - [`DomainOpError::InputUnbounded`] if either of the input domains are unbounded.
    ///  - [`DomainOpError::InputWrongType`] if the input domains are different types, or are not
    ///    integer or set domains.
    pub fn intersect(&self, other: &Domain) -> Result<Domain, DomainOpError> {
        // TODO: does not consider unbounded domains yet
        // needs to be tested once comprehension rules are written

        match (self, other) {
            // one or more arguments is an empty int domain
            (d @ Domain::Empty(ReturnType::Int), Domain::Int(_)) => Ok(d.clone()),
            (Domain::Int(_), d @ Domain::Empty(ReturnType::Int)) => Ok(d.clone()),
            (Domain::Empty(ReturnType::Int), d @ Domain::Empty(ReturnType::Int)) => Ok(d.clone()),

            // one or more arguments is an empty set(int) domain
            (Domain::Set(_, inner1), d @ Domain::Empty(ReturnType::Set(inner2)))
                if matches!(**inner1, Domain::Int(_) | Domain::Empty(ReturnType::Int))
                    && matches!(**inner2, ReturnType::Int) =>
            {
                Ok(d.clone())
            }
            (d @ Domain::Empty(ReturnType::Set(inner1)), Domain::Set(_, inner2))
                if matches!(**inner1, ReturnType::Int)
                    && matches!(**inner2, Domain::Int(_) | Domain::Empty(ReturnType::Int)) =>
            {
                Ok(d.clone())
            }
            (
                d @ Domain::Empty(ReturnType::Set(inner1)),
                Domain::Empty(ReturnType::Set(inner2)),
            ) if matches!(**inner1, ReturnType::Int) && matches!(**inner2, ReturnType::Int) => {
                Ok(d.clone())
            }

            // both arguments are non-empy
            (Domain::Set(_, x), Domain::Set(_, y)) => {
                Ok(Domain::Set(SetAttr::None, Box::new((*x).intersect(y)?)))
            }

            (Domain::Int(_), Domain::Int(_)) => {
                let mut v: BTreeSet<i32> = BTreeSet::new();

                let v1 = self.values_i32()?;
                let v2 = other.values_i32()?;
                for value1 in v1.iter() {
                    if v2.contains(value1) && !v.contains(value1) {
                        v.insert(*value1);
                    }
                }
                Ok(Domain::from_set_i32(&v))
            }
            _ => Err(DomainOpError::InputWrongType),
        }
    }

    /// Calculates the union of two domains.
    ///
    /// # Errors
    ///
    ///  - [`DomainOpError::InputUnbounded`] if either of the input domains are unbounded.
    ///  - [`DomainOpError::InputWrongType`] if the input domains are different types, or are not
    ///    integer set, or matrix domains. This is also thrown if the matrix domains that have
    ///    different index domains.
    ///
    pub fn union(&self, other: &Domain) -> Result<Domain, DomainOpError> {
        // TODO: does not consider unbounded domains yet
        // needs to be tested once comprehension rules are written
        match (self, other) {
            // one or more arguments is an empty integer domain
            (Domain::Empty(ReturnType::Int), d @ Domain::Int(_)) => Ok(d.clone()),
            (d @ Domain::Int(_), Domain::Empty(ReturnType::Int)) => Ok(d.clone()),
            (Domain::Empty(ReturnType::Int), d @ Domain::Empty(ReturnType::Int)) => Ok(d.clone()),

            // one or more arguments is an empty set(int) domain
            (d @ Domain::Set(_, inner1), Domain::Empty(ReturnType::Set(inner2)))
                if matches!(**inner1, Domain::Int(_) | Domain::Empty(ReturnType::Int))
                    && matches!(**inner2, ReturnType::Int) =>
            {
                Ok(d.clone())
            }
            (Domain::Empty(ReturnType::Set(inner1)), d @ Domain::Set(_, inner2))
                if matches!(**inner1, ReturnType::Int)
                    && matches!(**inner2, Domain::Int(_) | Domain::Empty(ReturnType::Int)) =>
            {
                Ok(d.clone())
            }
            (
                d @ Domain::Empty(ReturnType::Set(inner1)),
                Domain::Empty(ReturnType::Set(inner2)),
            ) if matches!(**inner1, ReturnType::Int) && matches!(**inner2, ReturnType::Int) => {
                Ok(d.clone())
            }

            // both arguments are non empty
            (Domain::Set(_, x), Domain::Set(_, y)) => {
                Ok(Domain::Set(SetAttr::None, Box::new((*x).union(y)?)))
            }

            // union matrices only if they have the same index domain.
            (Domain::Matrix(d1, idx1), Domain::Matrix(d2, idx2)) if idx1 == idx2 => Ok(
                Domain::Matrix(Box::new(d1.union(d2.as_ref())?), idx1.clone()),
            ),

            (Domain::Int(_), Domain::Int(_)) => {
                let mut v: BTreeSet<i32> = BTreeSet::new();
                let v1 = self.values_i32()?;
                let v2 = other.values_i32()?;

                for value1 in v1.iter() {
                    v.insert(*value1);
                }

                for value2 in v2.iter() {
                    v.insert(*value2);
                }

                Ok(Domain::from_set_i32(&v))
            }
            _ => Err(DomainOpError::InputWrongType),
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
            Domain::Reference(name) => write!(f, "{name}"),
            Domain::Set(_, domain) => {
                write!(f, "set of ({domain})")
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
            Domain::Empty(return_type) => write!(f, "empty({return_type:?}"),
        }
    }
}

impl Typeable for Domain {
    fn return_type(&self) -> Option<ReturnType> {
        match self {
            Domain::Bool => Some(ReturnType::Bool),
            Domain::Int(_) => Some(ReturnType::Int),
            Domain::Empty(return_type) => Some(return_type.clone()),
            Domain::Set(_, domain) => Some(ReturnType::Set(Box::new(domain.return_type()?))),
            Domain::Reference(_) => None, // todo!("add ReturnType for Domain::Reference"),
            Domain::Matrix(item_domain, index_domains) => {
                assert!(
                    !index_domains.is_empty(),
                    "a matrix should have atleast one dimension"
                );
                let mut return_type = ReturnType::Matrix(Box::new(item_domain.return_type()?));

                for _ in 0..(index_domains.len() - 1) {
                    return_type = ReturnType::Matrix(Box::new(return_type));
                }

                Some(return_type)
            }
            Domain::Tuple(items) => {
                let mut item_types = vec![];
                for item in items {
                    item_types.push(item.return_type()?);
                }
                Some(ReturnType::Tuple(item_types))
            }
            Domain::Record(items) => {
                let mut item_types = vec![];
                for item in items {
                    item_types.push(item.domain.return_type()?);
                }
                Some(ReturnType::Record(item_types))
            }
        }
    }
}

/// An error thrown by an operation on domains.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[allow(clippy::enum_variant_names)] // all variant names start with Input at the moment, but that is ok.
pub enum DomainOpError {
    /// The operation only supports bounded / finite domains, but was given an unbounded input domain.
    #[error(
        "The operation only supports bounded / finite domains, but was given an unbounded input domain."
    )]
    InputUnbounded,

    /// The operation only supports integer input domains, but was given an input domain of a
    /// different type.
    #[error("The operation only supports integer input domains, but got a {0:?} input domain.")]
    InputNotInteger(ReturnType),

    /// The operation was given an input domain of the wrong type.
    #[error("The operation was given input domains of the wrong type.")]
    InputWrongType,

    /// The operation failed as the input domain contained a reference.
    #[error("The operation failed as the input domain contained a reference")]
    InputContainsReference,
}

/// Types that have a [`Domain`].
pub trait HasDomain {
    /// Gets the [`Domain`] of `self`.
    fn domain_of(&self) -> Domain;

    /// Gets the [`Domain`] of `self`, replacing any references with their domains stored in from the symbol table.
    ///
    /// # Panics
    ///
    /// - If a symbol referenced in `self` does not exist in the symbol table.
    fn resolved_domain_of(&self, symbol_table: &SymbolTable) -> Domain {
        self.domain_of().resolve(symbol_table)
    }
}

impl<T: HasDomain> Typeable for T {
    fn return_type(&self) -> Option<ReturnType> {
        self.domain_of().return_type()
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
            assert!(!ranges.contains(&Range::Bounded(-4, 4)));
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
            assert!(!ranges.contains(&Range::Bounded(-4, 4)));
        }
    }
}

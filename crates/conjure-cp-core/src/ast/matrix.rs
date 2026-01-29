//! Utility functions for working with matrices.

// TODO: Georgiis essence macro would look really nice in these examples!

use std::collections::VecDeque;

use itertools::{Itertools, izip};
use uniplate::Uniplate as _;

use crate::ast::{DomainOpError, Expression as Expr, GroundDomain, Metadata, Moo, Range};

use super::{AbstractLiteral, Literal};

/// For some index domains, returns a list containing each of the possible indices.
///
/// Indices are traversed in row-major ordering.
///
/// This is an O(n^dim) operation, where dim is the number of dimensions in the matrix.
///
/// # Panics
///
/// + If any of the index domains are not finite or enumerable with [`Domain::values`].
///
/// # Example
///
/// ```
/// use std::collections::HashSet;
/// use conjure_cp_core::ast::{GroundDomain,Moo,Range,Literal,matrix};
/// let index_domains = vec![Moo::new(GroundDomain::Bool),Moo::new(GroundDomain::Int(vec![Range::Bounded(1,2)]))];
///
/// let expected_indices = HashSet::from([
///   vec![Literal::Bool(false),Literal::Int(1)],
///   vec![Literal::Bool(false),Literal::Int(2)],
///   vec![Literal::Bool(true),Literal::Int(1)],
///   vec![Literal::Bool(true),Literal::Int(2)]
///   ]);
///
/// let actual_indices: HashSet<_> = matrix::enumerate_indices(index_domains).collect();
///
/// assert_eq!(actual_indices, expected_indices);
/// ```
pub fn enumerate_indices(
    index_domains: Vec<Moo<GroundDomain>>,
) -> impl Iterator<Item = Vec<Literal>> {
    index_domains
        .into_iter()
        .map(|x| {
            x.values()
                .expect("index domain should be enumerable with .values()")
                .collect_vec()
        })
        .multi_cartesian_product()
}

/// Returns the number of possible elements indexable by the given index domains.
///
/// In short, returns the product of the sizes of the given indices.
pub fn num_elements(index_domains: &[Moo<GroundDomain>]) -> Result<u64, DomainOpError> {
    let idx_dom_lengths = index_domains
        .iter()
        .map(|d| d.length())
        .collect::<Result<Vec<_>, _>>()?;
    Ok(idx_dom_lengths.iter().product())
}

/// Flattens a multi-dimensional matrix literal into a one-dimensional slice of its elements.
///
/// The elements of the matrix are returned in row-major ordering (see [`enumerate_indices`]).
///
/// # Panics
///
/// + If the number or type of elements in each dimension is inconsistent.
///
/// + If `matrix` is not a matrix.
pub fn flatten(matrix: AbstractLiteral<Literal>) -> impl Iterator<Item = Literal> {
    let AbstractLiteral::Matrix(elems, _) = matrix else {
        panic!("matrix should be a matrix");
    };

    flatten_1(elems)
}

fn flatten_1(elems: Vec<Literal>) -> impl Iterator<Item = Literal> {
    elems.into_iter().flat_map(|elem| {
        if let Literal::AbstractLiteral(m @ AbstractLiteral::Matrix(_, _)) = elem {
            Box::new(flatten(m)) as Box<dyn Iterator<Item = Literal>>
        } else {
            Box::new(std::iter::once(elem)) as Box<dyn Iterator<Item = Literal>>
        }
    })
}
/// Flattens a multi-dimensional matrix literal into an iterator over (indices,element).
///
/// # Panics
///
///   + If the number or type of elements in each dimension is inconsistent.
///
///   + If `matrix` is not a matrix.
///
///   + If any dimensions in the matrix are not finite or enumerable with [`Domain::values`].
///     However, index domains in the form `int(i..)` are supported.
pub fn flatten_enumerate(
    matrix: AbstractLiteral<Literal>,
) -> impl Iterator<Item = (Vec<Literal>, Literal)> {
    let AbstractLiteral::Matrix(elems, _) = matrix.clone() else {
        panic!("matrix should be a matrix");
    };

    let index_domains = index_domains(matrix)
        .into_iter()
        .map(|mut x| match Moo::make_mut(&mut x) {
            // give unboundedr index domains an end
            GroundDomain::Int(ranges) if ranges.len() == 1 && !elems.is_empty() => {
                if let Range::UnboundedR(start) = ranges[0] {
                    ranges[0] = Range::Bounded(start, start + (elems.len() as i32 - 1));
                };
                x
            }
            _ => x,
        })
        .collect_vec();

    izip!(enumerate_indices(index_domains), flatten_1(elems))
}

/// Gets the index domains for a matrix literal.
///
/// # Panics
///
/// + If `matrix` is not a matrix.
///
/// + If the number or type of elements in each dimension is inconsistent.
pub fn index_domains(matrix: AbstractLiteral<Literal>) -> Vec<Moo<GroundDomain>> {
    let AbstractLiteral::Matrix(_, _) = matrix else {
        panic!("matrix should be a matrix");
    };

    matrix.cata(&move |element: AbstractLiteral<Literal>,
                       child_index_domains: VecDeque<Vec<Moo<GroundDomain>>>| {
        assert!(
            child_index_domains.iter().all_equal(),
            "each child of a matrix should have the same index domain"
        );

        let child_index_domains = child_index_domains
            .front()
            .unwrap_or(&vec![])
            .iter()
            .cloned()
            .collect_vec();
        match element {
            AbstractLiteral::Set(_) => vec![],
            AbstractLiteral::Matrix(_, domain) => {
                let mut index_domains = vec![domain];
                index_domains.extend(child_index_domains);
                index_domains
            }
            AbstractLiteral::Tuple(_) => vec![],
            AbstractLiteral::Record(_) => vec![],
            AbstractLiteral::Function(_) => vec![],
        }
    })
}

/// See [`enumerate_indices`]. This function zips the two given lists of index domains, performs a
/// union on each pair, and returns an enumerating iterator over the new list of domains.
pub fn enumerate_index_union_indices(
    a_domains: &[Moo<GroundDomain>],
    b_domains: &[Moo<GroundDomain>],
) -> Result<impl Iterator<Item = Vec<Literal>>, DomainOpError> {
    if a_domains.len() != b_domains.len() {
        return Err(DomainOpError::WrongType);
    }
    let idx_domains: Result<Vec<_>, _> = a_domains
        .iter()
        .zip(b_domains.iter())
        .map(|(a, b)| a.union(b))
        .collect();
    let idx_domains = idx_domains?.into_iter().map(Moo::new).collect();

    Ok(enumerate_indices(idx_domains))
}

// Given index domains for a multi-dimensional matrix and the nth index in the flattened matrix, find the coordinates in the original matrix
pub fn flat_index_to_full_index(index_domains: &[Moo<GroundDomain>], index: u64) -> Vec<Literal> {
    let mut remaining = index;
    let mut multipliers = vec![1; index_domains.len()];

    for i in (1..index_domains.len()).rev() {
        multipliers[i - 1] = multipliers[i] * index_domains[i].as_ref().length().unwrap();
    }

    let mut coords = Vec::new();
    for m in multipliers.iter() {
        // adjust for 1-based indexing
        coords.push(((remaining / m + 1) as i32).into());
        remaining %= *m;
    }

    coords
}

/// This is the same as `m[x]` except when `m` is of the forms:
///
/// - `n[..]`, then it produces n[x] instead of n[..][x]
/// - `flatten(n)`, then it produces `n[y]` instead of `flatten(n)[y]`,
///   where `y` is the full index corresponding to flat index `x`
///
/// # Returns
/// + `Some(expr)` if the safe indexing could be constructed
/// + `None` if it could not be constructed (e.g. invalid index type)
pub fn safe_index_optimised(m: Expr, idx: Literal) -> Option<Expr> {
    match m {
        Expr::SafeSlice(_, mat, idxs) => {
            // TODO: support >1 slice index (i.e. multidimensional slices)

            let mut idxs = idxs;
            let (slice_idx, _) = idxs.iter().find_position(|opt| opt.is_none())?;
            let _ = idxs[slice_idx].replace(idx.into());

            let Some(idxs) = idxs.into_iter().collect::<Option<Vec<_>>>() else {
                todo!("slice expression should not contain more than one unspecified index")
            };

            Some(Expr::SafeIndex(Metadata::new(), mat, idxs))
        }
        Expr::Flatten(_, None, inner) => {
            // Similar to indexed_flatten_matrix rule, but we don't care about out of bounds here
            let Literal::Int(index) = idx else {
                return None;
            };

            let dom = inner.domain_of().and_then(|dom| dom.resolve())?;
            let GroundDomain::Matrix(_, index_domains) = dom.as_ref() else {
                return None;
            };
            let flat_index = flat_index_to_full_index(index_domains, (index - 1) as u64);
            let flat_index: Vec<Expr> = flat_index.into_iter().map(Into::into).collect();

            Some(Expr::SafeIndex(Metadata::new(), inner, flat_index))
        }
        _ => Some(Expr::SafeIndex(
            Metadata::new(),
            Moo::new(m),
            vec![idx.into()],
        )),
    }
}

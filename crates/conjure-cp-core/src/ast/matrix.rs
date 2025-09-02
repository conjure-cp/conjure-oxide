//! Utility functions for working with matrices.

// TODO: Georgiis essence macro would look really nice in these examples!

use std::collections::VecDeque;

use itertools::{Itertools, izip};
use uniplate::Uniplate as _;

use crate::ast::{Domain, Range};

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
/// use conjure_cp_core::ast::{Domain,Range,Literal,matrix};
/// let index_domains = vec![Domain::Bool,Domain::Int(vec![Range::Bounded(1,2)])];
///
/// let expected_indices = vec![
///   vec![Literal::Bool(false),Literal::Int(1)],
///   vec![Literal::Bool(false),Literal::Int(2)],
///   vec![Literal::Bool(true),Literal::Int(1)],
///   vec![Literal::Bool(true),Literal::Int(2)]
///   ];
///
/// let actual_indices: Vec<_> = matrix::enumerate_indices(index_domains).collect();
///
/// assert_eq!(actual_indices, expected_indices);
/// ```
pub fn enumerate_indices(index_domains: Vec<Domain>) -> impl Iterator<Item = Vec<Literal>> {
    index_domains
        .into_iter()
        .map(|x| {
            x.values()
                .expect("index domain should be enumerable with .values()")
        })
        .multi_cartesian_product()
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
        .map(|mut x| match x {
            // give unboundedr index domains an end
            Domain::Int(ref mut ranges) if ranges.len() == 1 && !elems.is_empty() => {
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
pub fn index_domains(matrix: AbstractLiteral<Literal>) -> Vec<Domain> {
    let AbstractLiteral::Matrix(_, _) = matrix else {
        panic!("matrix should be a matrix");
    };

    matrix.cata(&move |element: AbstractLiteral<Literal>,
                       child_index_domains: VecDeque<Vec<Domain>>| {
        assert!(
            child_index_domains.iter().all_equal(),
            "each child of a matrix should have the same index domain"
        );

        let child_index_domains = child_index_domains.front().cloned().unwrap_or(vec![]);
        match element {
            AbstractLiteral::Set(_) => vec![],
            AbstractLiteral::Matrix(_, domain) => {
                let mut index_domains = vec![*domain];
                index_domains.extend(child_index_domains);
                index_domains
            }
            AbstractLiteral::Tuple(_) => vec![],
            AbstractLiteral::Record(_) => vec![],
        }
    })
}

//! Utility functions for working with matrices.

// TODO: Georgiis essence macro would look really nice in these examples!

use itertools::Itertools;

use crate::ast::Domain;

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
/// use conjure_core::ast::{Domain,Range,Literal,matrix};
/// let index_domains = vec![Domain::BoolDomain,Domain::IntDomain(vec![Range::Bounded(1,2)])];
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

    elems.into_iter().flat_map(|elem| {
        if let Literal::AbstractLiteral(m @ AbstractLiteral::Matrix(_, _)) = elem {
            Box::new(flatten(m)) as Box<dyn Iterator<Item = Literal>>
        } else {
            Box::new(std::iter::once(elem)) as Box<dyn Iterator<Item = Literal>>
        }
    })
}

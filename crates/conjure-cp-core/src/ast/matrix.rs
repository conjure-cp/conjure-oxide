//! Utility functions for working with matrices.
use std::collections::VecDeque;

use crate::ast::literals::AbstractLiteralValue;
use crate::ast::{
    AbstractLiteral, Atom, DomainOpError, DomainPtr, Expression as Expr, GroundDomain, Literal,
    Metadata, Moo, Range,
};
use crate::bug;
use crate::utils::MatrixShape;

use itertools::{Itertools, izip};
use uniplate::Biplate;

// ======================================================
// = "Shape" operations for matrices and matrix domains =
// ======================================================

/// Given an [AbstractLiteral::Matrix], get its [MatrixShape]
pub fn shape_of<T: MatrixValue>(matrix: &AbstractLiteral<T>) -> Option<MatrixShape<T::Dom>> {
    // put the dimensions in correct order
    let mut res = shape_of_inner(matrix)?;
    res.strides.reverse();
    res.dims.reverse();
    res.idx_doms.reverse();
    Some(res)
}

fn shape_of_inner<T: MatrixValue>(matrix: &AbstractLiteral<T>) -> Option<MatrixShape<T::Dom>> {
    let AbstractLiteral::Matrix(elems, dom) = matrix else {
        return None;
    };

    let sz = elems.len();
    if sz == 0 {
        return Some(MatrixShape {
            size: 0,
            strides: vec![0],
            dims: vec![0],
            idx_doms: vec![dom.clone()],
        });
    };

    // get child shapes and check that they are all the same
    let fst = elems[0].as_nested_matrix().and_then(shape_of_inner);
    for elem in elems.iter().skip(1) {
        debug_assert_eq!(
            fst,
            elem.as_nested_matrix().and_then(shape_of_inner),
            "Expected matrix elements to be consistent"
        );
    }

    Some(match fst {
        // if child shape is None, we have reached the last dimension
        None => MatrixShape {
            size: sz,
            strides: vec![1],
            dims: vec![sz],
            idx_doms: vec![dom.clone()],
        },
        // accumulate the next dimension
        Some(mut res) => {
            res.strides.push(res.size);
            res.dims.push(sz);
            res.idx_doms.push(dom.clone());
            res.size = if res.size == 0 { sz } else { sz * res.size };
            res
        }
    })
}

/// If this is a matrix expression (as defined by [Expr::unwrap_matrix_unchecked]),
/// get its [MatrixShape]. See also: [shape_of].
pub fn shape_of_matrix_expr(expr: &Expr) -> Option<MatrixShape<DomainPtr>> {
    match expr {
        Expr::Atomic(_, Atom::Literal(Literal::AbstractLiteral(lit))) => {
            Some(shape_of(lit)?.into())
        }
        Expr::AbstractLiteral(_, lit) => shape_of(lit),
        _ => None,
    }
}

/// Same as [shape_of] but for a ground matrix domain
pub fn shape_of_dom(
    matrix_dom_gd: &GroundDomain,
) -> Result<MatrixShape<Moo<GroundDomain>>, DomainOpError> {
    let GroundDomain::Matrix(_, idx_doms) = matrix_dom_gd else {
        return Err(DomainOpError::WrongType);
    };

    let len = idx_doms.len();
    let mut strides = VecDeque::with_capacity(len);
    let mut dimensions = VecDeque::with_capacity(len);

    let mut size: usize = 1;
    for gd in idx_doms.iter().rev() {
        let gd_sz = gd.len_usize()?;
        strides.push_front(size);
        dimensions.push_front(gd_sz);
        size = size.checked_mul(gd_sz).ok_or(DomainOpError::TooLarge)?;
    }

    Ok(MatrixShape {
        size,
        dims: dimensions.into(),
        strides: strides.into(),
        idx_doms: idx_doms.clone(),
    })
}

// ================================
// = Matrix flattening operations =
// ================================

/// Given a nested matrix, flatten its first `n+1` dimensions into one.
/// The resulting matrix will be a list because otherwise index domains would get weird, fast...
/// (unless we want to only support integer indices?)
pub fn partial_flatten<T: MatrixValue>(n: usize, matrix: AbstractLiteral<T>) -> AbstractLiteral<T> {
    if n == 0 {
        return matrix;
    }

    let shape = shape_of(&matrix).unwrap_or_else(|| bug!("Expected a matrix, got: {matrix}"));
    debug_assert!(
        n > 0 && n < shape.dims.len(),
        "Invalid number of dimensions to flatten"
    );
    let new_strides = Vec::from(&shape.strides[n..]);

    let flattened = flatten_owned(matrix).collect_vec();
    let res = unflatten_list(&flattened, &new_strides);

    res.into_nested_matrix()
        .unwrap_or_else(|e| bug!("Not a matrix: {e}"))
}

/// Flattens a multi-dimensional matrix into a one-dimensional slice of its elements.
/// The elements are returned in row-major ordering (see [`enumerate_indices`]).
/// The elements are borrowed; To consume the matrix and return owned values, see [flatten_owned].
///
/// # Panics
/// + If the number or type of elements in each dimension is inconsistent.
/// + If `matrix` is not a matrix.
pub fn flatten<T: MatrixValue>(matrix: &AbstractLiteral<T>) -> impl Iterator<Item = &T> {
    let AbstractLiteral::Matrix(elems, _) = matrix else {
        panic!("expected a matrix");
    };
    flatten_inner(elems)
}

#[inline]
fn flatten_inner<'a, T: MatrixValue>(elems: &'a [T]) -> impl Iterator<Item = &'a T> {
    elems.iter().flat_map(|elem| {
        if let Some(m) = elem.as_nested_matrix() {
            Box::new(flatten(m)) as Box<dyn Iterator<Item = &'a T>>
        } else {
            Box::new(std::iter::once(elem)) as Box<dyn Iterator<Item = &'a T>>
        }
    })
}

/// Consumes a multi-dimensional matrix and returns a one-dimensional slice of its elements.
/// The elements are returned in row-major ordering (see [`enumerate_indices`]).
///
/// # Panics
/// + If the number or type of elements in each dimension is inconsistent.
/// + If `matrix` is not a matrix.
pub fn flatten_owned<T: MatrixValue>(matrix: AbstractLiteral<T>) -> impl Iterator<Item = T> {
    let AbstractLiteral::Matrix(elems, _) = matrix else {
        panic!("expected a matrix");
    };
    flatten_owned_inner(elems)
}

#[inline]
fn flatten_owned_inner<T: MatrixValue>(elems: Vec<T>) -> impl Iterator<Item = T> {
    elems
        .into_iter()
        .flat_map(|elem| match elem.into_nested_matrix() {
            Ok(m) => Box::new(flatten_owned(m)) as Box<dyn Iterator<Item = T>>,
            Err(leaf) => Box::new(std::iter::once(leaf)) as Box<dyn Iterator<Item = T>>,
        })
}

// ====================================
// = Matrix "unflattening" operations =
// ====================================

/// "Un-flatten" a slice of elements into a Matrix with the given index domains
pub fn unflatten_matrix<T: MatrixValue>(
    elems: &[T],
    index_domains: &[T::Dom],
    strides: &[usize],
) -> T {
    let dom = index_domains.first().expect("no index domains").clone();
    let stride = *strides.first().expect("no strides");

    if index_domains.len() == 1 {
        return T::from(AbstractLiteral::Matrix(Vec::from(elems), dom));
    }

    let mut inners = Vec::<T>::with_capacity(stride);
    let mut i_start: usize = 0;
    while i_start < elems.len() {
        let next = i_start + stride;
        let elem = unflatten_matrix(&elems[i_start..next], &index_domains[1..], &strides[1..]);
        inners.push(elem);
        i_start = next;
    }
    T::from(AbstractLiteral::Matrix(inners, dom))
}

/// Same transformation as [unflatten_matrix], but all index domains become `int(1..)`
pub fn unflatten_list<T: MatrixValue>(elems: &[T], strides: &[usize]) -> T {
    let stride = *strides.first().expect("no strides");
    if strides.len() == 1 {
        return AbstractLiteral::matrix_implied_indices(Vec::from(elems)).into();
    }

    let mut inners = Vec::<T>::with_capacity(stride);
    let mut i_start: usize = 0;
    while i_start < elems.len() {
        let next = i_start + stride;
        let elem = unflatten_list(&elems[i_start..next], &strides[1..]);
        inners.push(elem);
        i_start = next;
    }
    AbstractLiteral::matrix_implied_indices(inners).into()
}

// =============================
// = Matrix indexing utilities =
// =============================

/// Gets the index domains for a matrix literal.
///
/// # Panics
///
/// + If `matrix` is not a matrix.
///
/// + If the number or type of elements in each dimension is inconsistent.
#[inline]
pub fn index_domains<T: MatrixValue>(matrix: &AbstractLiteral<T>) -> Vec<T::Dom> {
    shape_of(matrix)
        .unwrap_or_else(|| bug!("Expected matrix, got: {matrix}"))
        .idx_doms
}

/// Gets the index domains for a matrix expression and resolves them
pub fn resolved_index_domains(
    matrix: &AbstractLiteral<Expr>,
) -> Result<Vec<Moo<GroundDomain>>, DomainOpError> {
    index_domains(matrix)
        .into_iter()
        .map(|d| d.resolve())
        .try_collect()
}

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
pub fn try_enumerate_indices(
    index_domains: Vec<Moo<GroundDomain>>,
) -> Result<impl Iterator<Item = Vec<Literal>>, DomainOpError> {
    let domains = index_domains
        .into_iter()
        .map(|x| x.values().map(|values| values.collect_vec()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(domains.into_iter().multi_cartesian_product())
}

/// For some index domains, returns a list containing each of the possible indices.
///
/// See [`try_enumerate_indices`] for the fallible variant.
#[inline]
pub fn enumerate_indices(
    index_domains: Vec<Moo<GroundDomain>>,
) -> impl Iterator<Item = Vec<Literal>> {
    try_enumerate_indices(index_domains).expect("index domain should be enumerable with .values()")
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

/// Flattens a multi-dimensional matrix literal into an iterator over (indices,element).
///
/// # Panics
///
///   + If the number or type of elements in each dimension is inconsistent.
///
///   + If `matrix` is not a matrix.
///
///   + If any dimensions in the matrix are not finite or enumerable with [`Domain::values`].
pub fn flatten_enumerate(
    matrix: AbstractLiteral<Literal>,
) -> impl Iterator<Item = (Vec<Literal>, Literal)> {
    let shape = shape_of(&matrix).unwrap_or_else(|| bug!("Expected matrix, got: {matrix}"));
    let index_domains: Vec<Moo<GroundDomain>> = shape
        .idx_doms
        .into_iter()
        .zip(shape.dims)
        .map(|(domain, len)| bound_index_domain_from_length(domain, len))
        .collect();
    izip!(enumerate_indices(index_domains), flatten_owned(matrix))
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

    try_enumerate_indices(idx_domains)
}

/// Given index domains for a multi-dimensional matrix and
/// the nth index in the flattened matrix, find the coordinates in the original matrix
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

/// Gets concrete index domains for a matrix expression.
///
/// For matrix literals, right-unbounded integer index domains like `int(1..)` are bounded using
/// the literal's realised size in that dimension. For non-literals, this falls back to the
/// expression's resolved domain.
pub fn bound_index_domains_of_expr(expr: &Expr) -> Option<Vec<Moo<GroundDomain>>> {
    let dom = expr.domain_of().and_then(|dom| dom.resolve().ok())?;
    let GroundDomain::Matrix(_, index_domains) = dom.as_ref() else {
        return None;
    };

    let Some(dimension_lengths) = expr_matrix_dimension_lengths(expr) else {
        return Some(index_domains.clone());
    };

    assert_eq!(
        index_domains.len(),
        dimension_lengths.len(),
        "matrix literal domain rank should match its realised rank"
    );

    Some(
        index_domains
            .iter()
            .cloned()
            .zip(dimension_lengths)
            .map(|(domain, len)| bound_index_domain_from_length(domain, len))
            .collect(),
    )
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

            let index_domains = bound_index_domains_of_expr(inner.as_ref())?;
            if index_domains.iter().any(|domain| domain.length().is_err()) {
                return None;
            }
            let flat_index = flat_index_to_full_index(&index_domains, (index - 1) as u64);
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

// ====================
// = Internal helpers =
// ====================

/// If this is a matrix expression, get sizes along its dimensions
#[inline]
fn expr_matrix_dimension_lengths(expr: &Expr) -> Option<Vec<usize>> {
    Some(shape_of_matrix_expr(expr)?.dims)
}

/// Cap unbounded integer index domains to the given matrix dimension length.
#[inline]
fn bound_index_domain_from_length(mut domain: Moo<GroundDomain>, len: usize) -> Moo<GroundDomain> {
    match Moo::make_mut(&mut domain) {
        GroundDomain::Int(ranges) if ranges.len() == 1 && len > 0 => {
            ranges[0] = match &ranges[0] {
                Range::UnboundedR(start) => Range::Bounded(*start, start + (len as i32 - 1)),
                Range::Unbounded | Range::UnboundedL(_) => Range::Bounded(1, len as i32),
                Range::Bounded(low, high) => Range::Bounded(*low, *high),
                Range::Single(value) => Range::Single(*value),
            };
            domain
        }
        _ => domain,
    }
}

/// Things that can appear inside a matrix.
///
/// This is a helper trait to unify matrix operations on `Expression::AbstractLiteral`
/// and `AbstractLiteral<Literal>`
pub trait MatrixValue:
    AbstractLiteralValue + Sized + From<AbstractLiteral<Self>> + Biplate<AbstractLiteral<Self>>
{
    /// If this element is a nested matrix, return a reference to it
    fn as_nested_matrix(&self) -> Option<&AbstractLiteral<Self>>;
    /// If this element is a nested matrix, consume it and return the matrix
    fn into_nested_matrix(self) -> Result<AbstractLiteral<Self>, Self>;
}

impl MatrixValue for Literal {
    #[inline]
    fn as_nested_matrix(&self) -> Option<&AbstractLiteral<Literal>> {
        match self {
            Literal::AbstractLiteral(m @ AbstractLiteral::Matrix(..)) => Some(m),
            _ => None,
        }
    }

    #[inline]
    fn into_nested_matrix(self) -> Result<AbstractLiteral<Literal>, Self> {
        match self {
            Literal::AbstractLiteral(m @ AbstractLiteral::Matrix(..)) => Ok(m),
            other => Err(other),
        }
    }
}

impl MatrixValue for Expr {
    #[inline]
    fn as_nested_matrix(&self) -> Option<&AbstractLiteral<Expr>> {
        match self {
            Expr::AbstractLiteral(_, m @ AbstractLiteral::Matrix(..)) => Some(m),
            _ => None,
        }
    }

    #[inline]
    fn into_nested_matrix(self) -> Result<AbstractLiteral<Expr>, Self> {
        match self {
            Expr::AbstractLiteral(_, m @ AbstractLiteral::Matrix(..)) => Ok(m),
            other => Err(other),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MatrixShape<T> {
    // Total count of elements
    pub size: usize,
    // Sizes along each dimension
    pub dims: Vec<usize>,
    // Strides for each dimension
    pub strides: Vec<usize>,
    // Index domains for each dimension
    pub idx_doms: Vec<T>,
}

impl<T> From<MatrixShape<T>> for View {
    fn from(value: MatrixShape<T>) -> Self {
        Self {
            offset: 0,
            dims: value.dims,
            strides: value.strides,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// A view into a 1D slice that we can manipulate
pub struct View {
    // Number of elements to skip at the beginning
    pub offset: usize,
    // Sizes along each dimension
    pub dims: Vec<usize>,
    // Strides for each dimension
    pub strides: Vec<usize>,
}

impl View {
    pub fn new(offset: usize, dims: Vec<usize>, strides: Vec<usize>) -> Self {
        Self {
            offset,
            dims,
            strides,
        }
    }

    fn apply_impl<'a, T>(
        elems: &'a [T],
        offset: usize,
        dims: &[usize],
        strides: &[usize],
    ) -> Vec<&'a T> {
        assert_eq!(dims.len(), strides.len());

        if dims.is_empty() {
            return vec![&elems[offset]];
        }

        let mut ans: Vec<&'a T> = Vec::new();
        for i in 0..dims[0] {
            let new_off = offset + i * strides[0];
            ans.extend(View::apply_impl(elems, new_off, &dims[1..], &strides[1..]));
        }
        ans
    }

    /// Get this view into `elems`
    pub fn apply<'a, T>(&self, elems: &'a [T]) -> Vec<&'a T> {
        View::apply_impl(elems, self.offset, &self.dims, &self.strides)
    }

    /// Reorder dimensions according to the given permutation.
    ///
    /// `perm` must be a permutation of `0..self.dims.len()`. The returned view
    /// iterates through the same backing store but with reordered dimensions:
    /// dimension `i` of the new view corresponds to dimension `perm[i]` of `self`.
    pub fn permute(&self, perm: &[usize]) -> View {
        debug_assert_eq!(perm.len(), self.dims.len());
        View::new(
            self.offset,
            perm.iter().map(|&i| self.dims[i]).collect(),
            perm.iter().map(|&i| self.strides[i]).collect(),
        )
    }

    /// Compute standard row-major strides for the given dimension sizes.
    ///
    /// For dims `[d0, d1, .., dN]` the strides are
    /// `[d1*d2*..*dN, d2*..*dN, .., 1]`.
    pub fn row_major_strides(dims: &[usize]) -> Vec<usize> {
        let mut strides = vec![1usize; dims.len()];
        for i in (0..dims.len().saturating_sub(1)).rev() {
            strides[i] = strides[i + 1] * dims[i + 1];
        }
        strides
    }
}

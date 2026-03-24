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
}

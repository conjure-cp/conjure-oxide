use super::prelude::*;
use crate::representation::prelude::matrix::{flatten, unflatten_matrix};
use bimap::BiMap;
use conjure_cp::ast::{GroundDomain, Moo, Range};
use conjure_cp::representation::ReprInitError;
use conjure_cp::utils::View;
use std::collections::VecDeque;

register_representation!(
    MatrixToAtom
    struct State<T> {
        // Size of each dimension
        pub dimensions: Vec<usize>,
        // Offsets into the flat vector to get to next element along this dimension
        pub strides: Vec<usize>,
        // Index domains of the original matrix
        pub index_domains: Vec<Moo<GroundDomain>>,
        // Map all possible indices to integers
        pub indices: Vec<BiMap<usize, Literal>>,
        // Flat vec of matrix elements
        pub elements: Vec<T>,
    }
    impl<T> State<T>
    {
        /// Number of elements in this matrix representation
        pub fn len(&self) -> usize {
            self.elements.len()
        }
        /// True if the representation of this matrix has size 0
        pub fn is_empty(&self) -> bool {
            self.elements.is_empty()
        }
        /// Convert a matrix index to flat integer index into `elements`
        pub fn indices_lits_to_flat(&self, idx: &[Literal]) -> Option<usize> {
            let mut ans: usize = 0;
            for (i, lit) in idx.iter().enumerate() {
                let flat = self.indices[i].get_by_right(lit)?;
                ans += flat * self.strides[i];
            }
            Some(ans)
        }
        /// Convert a flat index into the original matrix index
        pub fn indices_flat_to_lits(&self, mut idx: usize) -> Vec<Literal> {
            let mut ans: Vec<Literal> = Vec::new();
            for (i, s) in self.strides.iter().copied().enumerate() {
                let dim_idx = idx / s;
                let dim_idx_lit = self.index_flat_to_lit(i, dim_idx);
                ans.push(dim_idx_lit.clone());
                idx %= s;
            }
            ans
        }
        /// Convert a flat index along the given dimension to a Literal
        pub fn index_flat_to_lit(&self, dim: usize, idx: usize) -> &Literal {
            self.indices[dim].get_by_left(&idx).expect("invalid index")
        }
        /// Convert a literal index along the given dimension to a flat index
        pub fn index_lit_to_flat(&self, dim: usize, lit: &Literal) -> usize {
            *self.indices[dim].get_by_right(lit).expect("invalid index")
        }
        /// Subset of `elements` corresponding to a matrix with the given dimensions and strides
        pub fn view<'a>(&'a self, view: &View) -> Vec<&'a T> {
            view.apply(&self.elements)
        }
        /// Copy of the subset of `elements` corresponding to a matrix with the given dimensions and strides
        pub fn view_cloned(&self, view: &View) -> Vec<T>
        where T: Clone
        {
            self.view(view).into_iter().cloned().collect()
        }
        /// Slice into the elements matrix via flat indices along each dimension
        pub fn slice_flat(&self, dim_slices: &[Range<usize>]) -> View {
            let mut offset = 0;
            let mut new_dims = Vec::new();
            let mut new_strides = Vec::new();
            for (dim, rng) in dim_slices.iter().enumerate() {
                let lo = rng.low().copied().unwrap_or(0);
                let hi = rng.high().copied().unwrap_or(self.dimensions[dim] - 1);
                // ranges are inclusive
                let dim_sz = hi - lo + 1;

                offset += self.strides[dim] * lo;
                if dim_sz > 1 {
                    new_dims.push(dim_sz);
                    new_strides.push(self.strides[dim]);
                }
            }
            View::new(offset, new_dims, new_strides)
        }
        /// Slice into the elements matrix via literal indices
        pub fn slice_lit(&self, dim_slices: &[Range<Literal>]) -> View {
            let dim_slices_flat = dim_slices.iter()
                .enumerate()
                .map(|(i, rng)|
                    rng.map(|l|
                        self.index_lit_to_flat(i, &l)))
                .collect::<Vec<_>>();
            self.slice_flat(&dim_slices_flat)
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(dom.clone(), MatrixToAtom::NAME, String::from(msg));

        let Some((inner_gd, index_gds)) = dom.as_matrix_ground() else {
            return Err(domain_err("expected ground matrix"));
        };

        let len = index_gds.len();
        let mut index_domains = VecDeque::with_capacity(len);
        let mut strides = VecDeque::with_capacity(len);
        let mut indices = VecDeque::with_capacity(len);
        let mut dimensions = VecDeque::with_capacity(len);

        let mut size: usize = 1;
        for gd in index_gds.iter().rev() {
            let gd_sz = gd.len_usize().ok().ok_or(domain_err("domain too large"))?;
            let gd_vals = gd.values().ok().ok_or(domain_err("domain not enumerable"))?;
            let gd_idx = BiMap::from_iter(gd_vals.enumerate());

            index_domains.push_front(gd.clone());
            strides.push_front(size);
            indices.push_front(gd_idx);
            dimensions.push_front(gd_sz);

            size = size.checked_mul(gd_sz).ok_or(domain_err("total size of the matrix is too large"))?;
        }

        let mut elements = Vec::new();
        for _ in 0..size {
            elements.push(inner_gd.clone().into());
        }

        Ok(State {
            elements,
            indices: indices.into(),
            index_domains: index_domains.into(),
            strides: strides.into(),
            dimensions: dimensions.into(),
        })
    }
    fn structural(_state: &State<DeclarationPtr>) -> Vec<Expression> {
        vec![]
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(abslit @ AbstractLiteral::Matrix(..)) = &value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a matrix literal")));
        };

        let elements: Vec<Literal> = flatten(abslit).cloned().collect();
        if elements.len() != state.elements.len() {
            let msg = format!(
                "expected {} elements, got {}",
                state.elements.len(),
                elements.len()
            );
            return Err(ReprDownError::BadValue(value, msg));
        }

        Ok(State {
            index_domains: state.index_domains.clone(),
            strides: state.strides.clone(),
            indices: state.indices.clone(),
            dimensions: state.dimensions.clone(),
            elements,
        })
    }
    fn up(state: State<Literal>) -> Literal {
        unflatten_matrix(&state.elements, &state.index_domains, &state.strides)
    }
);

use super::prelude::*;
use crate::representation::prelude::matrix::{flatten, unflatten_matrix};
use conjure_cp::ast::matrix::shape_of_dom;
use conjure_cp::ast::{GroundDomain, Moo, Range, Reference};
use conjure_cp::representation::ReprInitError;
use conjure_cp::utils::{BiMap, MatrixShape, View};
use itertools::Itertools;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum IdxError {
    #[error("{1} is not a valid index for dimension {0}")]
    Lit(usize, Literal),
    #[error("{1} is not a valid index for dimension {0}")]
    Int(usize, usize),
}

register_representation!(
    MatrixToAtom
    struct State<T> {
        // Size of each dimension
        pub dimensions: Vec<usize>,
        // Offsets into the flat vector to get to next element along this dimension
        pub strides: Vec<usize>,
        // Index domains of the original matrix
        pub index_domains: Vec<Moo<GroundDomain>>,
        // -- Larger fields are wrapped in Moo to avoid cloning them --
        // Map all possible indices to integers
        pub indices: Moo<Vec<BiMap<usize, Literal>>>,
        // Flat vec of matrix elements
        pub elements: Moo<Vec<T>>,
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
        pub fn indices_lits_to_flat(&self, idx: &[Literal]) -> Result<usize, IdxError> {
            let mut ans: usize = 0;
            for (i, lit) in idx.iter().enumerate() {
                let flat = self.index_lit_to_flat(i, lit)?;
                ans += flat * self.strides[i];
            }
            Ok(ans)
        }
        /// Convert a flat index into the original matrix index
        pub fn indices_flat_to_lits(&self, mut idx: usize) -> Result<Vec<Literal>, IdxError> {
            let mut ans: Vec<Literal> = Vec::new();
            for (i, s) in self.strides.iter().copied().enumerate() {
                let dim_idx = idx / s;
                let dim_idx_lit = self.index_flat_to_lit(i, dim_idx)?;
                ans.push(dim_idx_lit.clone());
                idx %= s;
            }
            Ok(ans)
        }
        /// Convert a flat index along the given dimension to a Literal
        pub fn index_flat_to_lit(&self, dim: usize, idx: usize) -> Result<&Literal, IdxError> {
            self.indices[dim].get_by_left(&idx).ok_or(IdxError::Int(dim, idx))
        }
        /// Convert a literal index along the given dimension to a flat index
        pub fn index_lit_to_flat(&self, dim: usize, lit: &Literal) -> Result<usize, IdxError> {
            self.indices[dim].get_by_right(lit).copied().ok_or(IdxError::Lit(dim, lit.clone()))
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
        /// Flatten the first n dimensions (inclusive)
        pub fn flatten(&self, n: usize) -> View {
            let new_strides = Vec::from(&self.strides[n..]);

            let mut new_dims = Vec::<usize>::with_capacity(self.dimensions.len() - n);
            new_dims.push(self.dimensions[0..n + 1].iter().copied().product());
            new_dims.extend(&self.dimensions[n + 1..]);

            View::new(0, new_dims, new_strides)
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
        pub fn slice_lit(&self, dim_slices: &[Range<Literal>]) -> Result<View, IdxError> {
            let dim_slices_flat: Vec<Range<usize>> = dim_slices.iter()
                .enumerate()
                .map(|(i, rng)|
                    rng.try_map(|l|
                        self.index_lit_to_flat(i, &l)))
                .try_collect()?;
            Ok(self.slice_flat(&dim_slices_flat))
        }
    }
    impl State<DeclarationPtr> {
        pub fn flat_elem_refs(&self) -> impl Iterator<Item=Reference> + '_ {
            self.elements.iter().cloned().map(Reference::new)
        }
        /// Get elements selected by a [`View`] as reference expressions.
        pub fn view_as_exprs(&self, view: &View) -> Vec<Expression> {
            self.view_cloned(view)
                .into_iter()
                .map(|decl| Expression::from(Reference::new(decl)))
                .collect()
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(dom.clone(), MatrixToAtom::NAME, String::from(msg));

        let dom_gd = dom.resolve().ok().ok_or(domain_err("expected a ground domain"))?;
        let GroundDomain::Matrix(elem_dom, _) = dom_gd.as_ref() else {
            return Err(domain_err("expected a matrix domain"));
        };
        let MatrixShape { size, dims, strides, idx_doms } = shape_of_dom(dom_gd.as_ref()).ok().ok_or(domain_err("expected a matrix domain"))?;

        let mut indices = Vec::with_capacity(dims.len());
        for dom in idx_doms.iter() {
            let vals = dom.values().map_err(|e| domain_err(&format!("could not enumerate index domain: {e}")))?;
            indices.push(BiMap::from_iter(vals.enumerate()))
        }

        let elements = vec![DomainPtr::from(elem_dom.clone()); size];

        Ok(State {
            elements: Moo::new(elements),
            indices: Moo::new(indices),
            index_domains: idx_doms,
            dimensions: dims,
            strides
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
            elements: Moo::new(elements)
        })
    }
    fn up(state: State<Literal>) -> Literal {
        unflatten_matrix(&state.elements, &state.index_domains, &state.strides)
    }
);

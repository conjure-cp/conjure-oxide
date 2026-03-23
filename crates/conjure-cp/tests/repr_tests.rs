//! Test that `register_representation!` generates the same code as the manually-written test_mta.rs

use bimap::BiMap;
use conjure_cp::ast::matrix::unflatten_matrix;
use conjure_cp::ast::{AbstractLiteral, GroundDomain, Moo};
use conjure_cp::utils::View;
use conjure_cp_core::ast::Range;
use conjure_cp_core::ast::matrix::flatten;
use std::collections::VecDeque;

conjure_cp::representation::register_representation!(
    MatrixToAtomMacro
    struct State<T> {
        // Size of each dimension
        dimensions: Vec<usize>,
        // Offsets into the flat vector to get to next element along this dimension
        strides: Vec<usize>,
        // Index domains of the original matrix
        index_domains: Vec<Moo<GroundDomain>>,
        // Map all possible indices to integers
        // TODO: this is only really used at decl level.
        //       we should rethink how the macros work so the user isn't forced to clone this
        //       every time for assignments etc
        indices: Vec<BiMap<usize, Literal>>,
        // Flat vec of matrix elements
        elements: Vec<T>,
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
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprError> {
        let Some((inner_gd, index_gds)) = dom.as_matrix_ground() else {
            return Err("Expected ground matrix".into());
        };

        let len = index_gds.len();
        let mut index_domains = VecDeque::with_capacity(len);
        let mut strides = VecDeque::with_capacity(len);
        let mut indices = VecDeque::with_capacity(len);
        let mut dimensions = VecDeque::with_capacity(len);

        let mut size: usize = 1;
        for gd in index_gds.iter().rev() {
            let gd_sz = gd.len_usize().ok().ok_or("overflow")?;
            let gd_vals = gd.values().map_err(|_| "Expected indices to be enumerable")?;
            let gd_idx = BiMap::from_iter(gd_vals.enumerate());

            index_domains.push_front(gd.clone());
            strides.push_front(size);
            indices.push_front(gd_idx);
            dimensions.push_front(gd_sz);

            size = size.checked_mul(gd_sz).ok_or("overflow")?;
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
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprError> {
        let Literal::AbstractLiteral(abslit) = value else {
            return Err(format!("expected matrix, got {:?}", value));
        };

        if !matches!(abslit, AbstractLiteral::Matrix(..)) {
            return Err(format!("expected matrix, got {:?}", abslit));
        }

        let elements: Vec<Literal> = flatten(abslit).collect();
        if elements.len() != state.elements.len() {
            return Err(format!(
                "expected {} elements, got {}",
                state.elements.len(),
                elements.len()
            ));
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

#[cfg(test)]
mod test {
    use super::*;
    use conjure_cp::ast::SymbolTable;
    use conjure_cp::ast::{HasDomain, Name};
    use conjure_cp::{domain_int, domain_int_ground, matrix_lit};
    use conjure_cp_core::ast::Domain;

    fn as_lits<I, T>(itr: I) -> Vec<Literal>
    where
        I: IntoIterator<Item = T>,
        Literal: From<T>,
    {
        itr.into_iter().map(Literal::from).collect()
    }

    #[test]
    fn test_matrix_int_3d_macro() {
        let expected = matrix_lit![
            [
                [1, 2, 3, 4],
                [5, 6, 7, 8],
                [9, 10, 11, 12]
            ],
            [
                [13, 14, 15, 16],
                [17, 18, 19, 20],
                [21, 22, 23, 24]
            ];
            [
                domain_int_ground!(1..2),
                domain_int_ground!(1..3),
                domain_int_ground!(1..4)
            ]
        ];

        let dom = expected.domain_of();

        let mut symtab = SymbolTable::new();
        let mut var = symtab.gensym(&dom);

        // Initialise the representation
        let (symbols, constraints) = MatrixToAtomMacro
            .init_for(&mut var)
            .expect("rule to apply successfully");

        assert_eq!(constraints, vec![]);
        let mut n_symbols = 0;
        for (name, decl) in symbols.iter_local() {
            let aux = decl.as_find().unwrap();
            assert_eq!(aux.domain, domain_int!(1..24));
            assert!(matches!(name, Name::Repr(_)));
            n_symbols += 1;
        }
        assert_eq!(n_symbols, 24);

        // Get the representation
        let repr = var
            .get_repr::<MatrixToAtomMacro>()
            .expect("State to be stored");

        // Check that all the bookkeeping vars were initialised correctly
        assert_eq!(repr.dimensions, vec![2, 3, 4]);
        assert_eq!(repr.strides, vec![12, 4, 1]);

        // Go down
        let down = repr.down(expected.clone()).expect("down");

        // Go back up
        let res = down.up();
        assert_eq!(res, expected);
    }

    #[test]
    fn test_idx() {
        /*
             1  3  5  7
        [
            [A, B, C, D],  - false
            [E, F, G, H]   - true
        ]
         */
        let dom = Domain::matrix(
            Domain::bool(),
            vec![Domain::bool(), domain_int!(1, 3, 5, 7)],
        );
        let repr = <MatrixToAtomMacro as ReprRule>::DomainLevel::init(dom).unwrap();

        assert_eq!(repr.dimensions, vec![2, 4]);
        assert_eq!(repr.strides, vec![4, 1]);

        let idx_lit = vec![Literal::from(false), Literal::from(1)];
        assert_eq!(repr.indices_lits_to_flat(&idx_lit), Some(0));
        assert_eq!(repr.indices_flat_to_lits(0), idx_lit);

        let idx_lit = vec![Literal::from(false), Literal::from(5)];
        assert_eq!(repr.indices_lits_to_flat(&idx_lit), Some(2));
        assert_eq!(repr.indices_flat_to_lits(2), idx_lit);

        let idx_lit = vec![Literal::from(false), Literal::from(7)];
        assert_eq!(repr.indices_lits_to_flat(&idx_lit), Some(3));
        assert_eq!(repr.indices_flat_to_lits(3), idx_lit);

        let idx_lit = vec![Literal::from(true), Literal::from(1)];
        assert_eq!(repr.indices_lits_to_flat(&idx_lit), Some(4));
        assert_eq!(repr.indices_flat_to_lits(4), idx_lit);

        let idx_lit = vec![Literal::from(true), Literal::from(5)];
        assert_eq!(repr.indices_lits_to_flat(&idx_lit), Some(6));
        assert_eq!(repr.indices_flat_to_lits(6), idx_lit);

        let idx_lit = vec![Literal::from(true), Literal::from(7)];
        assert_eq!(repr.indices_lits_to_flat(&idx_lit), Some(7));
        assert_eq!(repr.indices_flat_to_lits(7), idx_lit);
    }

    #[test]
    fn test_view() {
        let tensor = matrix_lit![
            [
                [1, 2, 3, 4],
                [5, 6, 7, 8],
                [9, 10, 11, 12]
            ],
            [
                [13, 14, 15, 16],
                [17, 18, 19, 20],
                [21, 22, 23, 24]
            ];
            [
                domain_int_ground!(1..2),
                domain_int_ground!(1..3),
                domain_int_ground!(1..4)
            ]
        ];

        let dom = tensor.domain_of();
        let mut symtab = SymbolTable::new();
        let mut var = symtab.gensym(&dom);
        let _ = MatrixToAtomMacro
            .init_for(&mut var)
            .expect("rule to apply successfully");
        let repr = var
            .get_repr::<MatrixToAtomMacro>()
            .expect("State to be stored");
        let down = repr.down(tensor.clone()).expect("down");

        let exp_odds = as_lits((1..25).filter(|x| x % 2 == 1));
        let odds = down.view_cloned(&View::new(0, vec![12], vec![2]));
        assert_eq!(exp_odds, odds);

        let exp_evens = as_lits((1..25).filter(|x| x % 2 == 0));
        let evens = down.view_cloned(&View::new(1, vec![12], vec![2]));
        assert_eq!(exp_evens, evens);

        let exp_half_oddrows = as_lits(vec![1, 2, 9, 10, 17, 18]);
        let half_oddrows = down.view_cloned(&View::new(0, vec![3, 2], vec![8, 1]));
        assert_eq!(exp_half_oddrows, half_oddrows);
    }

    #[test]
    fn test_slice() {
        let tensor = matrix_lit![
            [
                [1, 2, 3, 4],
                [5, 6, 7, 8],
                [9, 10, 11, 12]
            ],
            [
                [13, 14, 15, 16],
                [17, 18, 19, 20],
                [21, 22, 23, 24]
            ];
            [
                domain_int_ground!(1..2),
                domain_int_ground!(1..3),
                domain_int_ground!(1..4)
            ]
        ];

        let dom = tensor.domain_of();
        let mut symtab = SymbolTable::new();
        let mut var = symtab.gensym(&dom);
        let _ = MatrixToAtomMacro
            .init_for(&mut var)
            .expect("rule to apply successfully");
        let repr = var
            .get_repr::<MatrixToAtomMacro>()
            .expect("State to be stored");
        let down = repr.down(tensor).expect("down");

        let slices = vec![Range::Single(1), Range::Unbounded, Range::Single(2)];
        let expected = as_lits(vec![15, 19, 23]);
        let view = down.slice_flat(&slices);
        let actual = down.view_cloned(&view);
        assert_eq!(expected, actual);

        let slices = vec![Range::Single(1), Range::Unbounded, Range::Bounded(1, 2)];
        let expected = as_lits(vec![14, 15, 18, 19, 22, 23]);
        let view = down.slice_flat(&slices);
        let actual = down.view_cloned(&view);
        assert_eq!(expected, actual);

        let lit_slices = vec![
            Range::Single(Literal::from(1)),
            Range::UnboundedL(Literal::from(2)),
            Range::Bounded(Literal::from(2), Literal::from(3)),
        ];
        let expected = as_lits(vec![2, 3, 6, 7]);
        let view = down.slice_lit(&lit_slices);
        let actual = down.view_cloned(&view);
        assert_eq!(expected, actual);
    }
}

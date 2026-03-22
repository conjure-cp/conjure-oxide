//! Test that `register_representation!` generates the same code as the manually-written test_mta.rs

use conjure_cp::ast::SymbolTable;
use conjure_cp::ast::{HasDomain, Name};
use conjure_cp::utils::View;
use conjure_cp::{domain_int, domain_int_ground, matrix_lit};
use conjure_cp_core::ast::Range;
use conjure_cp_core::ast::{Domain, Literal};
use conjure_cp_core::representation::{ReprAssignment, ReprDeclLevel, ReprDomainLevel, ReprRule};
use conjure_cp_rules::representation::MatrixToAtom;
use std::collections::HashSet;

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
    let (symbols, constraints) =
        MatrixToAtom::init_for(&mut var).expect("rule to apply successfully");
    assert_eq!(constraints, vec![]);

    #[allow(clippy::mutable_key_type)]
    let mut repr_vars = HashSet::new();
    for (name, decl) in symbols.iter_local() {
        let aux = decl.as_find().unwrap();
        assert_eq!(aux.domain, domain_int!(1..24));
        assert!(matches!(name, Name::Repr(_)));
        repr_vars.insert(decl.clone());
    }
    assert_eq!(repr_vars.len(), 24);

    // Get the representation
    let repr = var.get_repr::<MatrixToAtom>().expect("State to be stored");
    assert_eq!(repr_vars, repr.repr_vars().into_iter().collect());

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
    let repr = <MatrixToAtom as ReprRule>::DomainLevel::init(dom).unwrap();

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
    let _ = MatrixToAtom::init_for(&mut var).expect("rule to apply successfully");
    let repr = var.get_repr::<MatrixToAtom>().expect("State to be stored");
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
    let _ = MatrixToAtom::init_for(&mut var).expect("rule to apply successfully");
    let repr = var.get_repr::<MatrixToAtom>().expect("State to be stored");
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

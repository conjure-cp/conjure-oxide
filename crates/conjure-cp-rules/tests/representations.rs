use conjure_cp::ast::records::Field;
use conjure_cp::ast::{
    AbstractLiteral, Domain, Expression, GroundDomain, Literal, MSetAttr, Metadata, Moo, Name,
    Reference, SetAttr, SymbolTable, run_partial_evaluator,
};
use conjure_cp::representation::{ReprAssignment, ReprDomainLevel, ReprRule};
use conjure_cp::rule_engine::ApplicationError::RuleNotApplicable;
use conjure_cp::{domain_int, range};
use conjure_cp_rules::representation::{
    MSetExplicit, MSetOccurrence, MSetPacked, MatrixComponents, MatrixPacked, RecordToTuple,
    SetExplicit, SetOccurrence, SetPacked, TupleComponents, TuplePacked,
};
use uniplate::Uniplate;

#[test]
fn representation_short_names_describe_the_generated_layout() {
    assert_eq!(MatrixComponents::SHORT_NAME, "components");
    assert_eq!(MatrixPacked::SHORT_NAME, "packed");
    assert_eq!(TupleComponents::SHORT_NAME, "components");
    assert_eq!(SetPacked::SHORT_NAME, "packed");
    assert_eq!(TuplePacked::SHORT_NAME, "packed");
    assert_eq!(SetExplicit::SHORT_NAME, "explicit");
    assert_eq!(SetOccurrence::SHORT_NAME, "occurrence");
    assert_eq!(RecordToTuple::SHORT_NAME, "tuple");
}

#[test]
fn packed_matrix_round_trips_primitive_values_and_weird_indices() {
    let inner_indices = GroundDomain::Int(vec![
        conjure_cp::ast::Range::Single(1),
        conjure_cp::ast::Range::Single(3),
    ]);
    let domain = Domain::matrix(
        domain_int!(2..4),
        vec![Domain::bool(), Domain::from(inner_indices.clone()).into()],
    );
    let row = |values| {
        Literal::AbstractLiteral(AbstractLiteral::Matrix(
            values,
            Moo::new(inner_indices.clone()),
        ))
    };
    let value = Literal::AbstractLiteral(AbstractLiteral::Matrix(
        vec![
            row(vec![Literal::Int(2), Literal::Int(4)]),
            row(vec![Literal::Int(3), Literal::Int(2)]),
        ],
        Moo::new(GroundDomain::Bool),
    ));

    let state = <MatrixPacked as ReprRule>::DomainLevel::init(domain.clone()).unwrap();
    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.packed, Literal::Int(21));
    assert_eq!(assignment.up(), value);
    assert_eq!(MatrixPacked::compactness_score(domain.clone()).unwrap(), 81);

    let mut symbols = SymbolTable::new();
    let mut declaration = symbols.gen_find(&domain);
    let (new_symbols, constraints) = MatrixPacked::init_for(&mut declaration).unwrap();
    assert_eq!(new_symbols.iter_local().count(), 1);
    assert!(constraints.is_empty());
    let decoded = declaration
        .get_repr::<MatrixPacked>()
        .unwrap()
        .decoded_matrix();
    assert_eq!(
        decoded
            .universe()
            .iter()
            .filter(|expr| matches!(expr, Expression::SafeIndex(..)))
            .count(),
        4
    );
}

#[test]
fn packed_matrix_uses_conjure_boolean_symmetry_order() {
    let domain = Domain::matrix(Domain::bool(), vec![domain_int!(1..2)]);
    let state = <MatrixPacked as ReprRule>::DomainLevel::init(domain).unwrap();
    assert_eq!(
        state.values.as_ref(),
        &[Literal::Bool(true), Literal::Bool(false)]
    );
    let value = Literal::AbstractLiteral(AbstractLiteral::Matrix(
        vec![Literal::Bool(true), Literal::Bool(false)],
        Moo::new(GroundDomain::Int(vec![conjure_cp::ast::Range::Bounded(
            1, 2,
        )])),
    ));
    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.packed, Literal::Int(1));
    assert_eq!(assignment.up(), value);
}

#[test]
fn matrix_representation_initialises_and_maps_indices() {
    let domain = Domain::matrix(
        Domain::bool(),
        vec![Domain::bool(), domain_int!(1, 3, 5, 7)],
    );

    let domain_state = <MatrixComponents as ReprRule>::DomainLevel::init(domain.clone()).unwrap();
    assert_eq!(domain_state.dimensions, vec![2, 4]);
    assert_eq!(domain_state.strides, vec![4, 1]);
    assert_eq!(
        domain_state
            .indices_lits_to_flat(&[Literal::Bool(true), Literal::Int(5)])
            .unwrap(),
        6
    );

    let mut symbols = SymbolTable::new();
    let mut declaration = symbols.gen_find(&domain);
    let (new_symbols, constraints) = MatrixComponents::init_for(&mut declaration).unwrap();
    assert!(constraints.is_empty());
    assert_eq!(new_symbols.iter_local().count(), 8);
    assert!(
        new_symbols
            .iter_local()
            .all(|(name, _)| name.to_string().contains("#components_"))
    );
    assert!(declaration.get_repr::<MatrixComponents>().is_some());

    let detached = declaration.detach();
    assert!(detached.get_repr::<MatrixComponents>().is_some());
}

#[test]
fn record_to_tuple_round_trips_values_in_field_order() {
    let domain = Domain::record(vec![
        Field {
            name: Name::user("z"),
            value: Domain::bool(),
        },
        Field {
            name: Name::user("a"),
            value: domain_int!(1..3),
        },
    ]);
    let state = <RecordToTuple as ReprRule>::DomainLevel::init(domain).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::Record(vec![
        Field {
            name: Name::user("z"),
            value: Literal::Bool(true),
        },
        Field {
            name: Name::user("a"),
            value: Literal::Int(2),
        },
    ]));

    let assignment = state.down(value).unwrap();
    assert_eq!(
        assignment.up(),
        Literal::AbstractLiteral(AbstractLiteral::Record(vec![
            Field {
                name: Name::user("a"),
                value: Literal::Int(2),
            },
            Field {
                name: Name::user("z"),
                value: Literal::Bool(true),
            },
        ]))
    );
}

#[test]
fn represented_record_equal_to_tuple_is_not_folded_to_false() {
    let domain = Domain::record(vec![
        Field {
            name: Name::user("a"),
            value: Domain::bool(),
        },
        Field {
            name: Name::user("b"),
            value: domain_int!(0..9),
        },
    ]);
    let mut symbols = SymbolTable::new();
    let mut declaration = symbols.gen_find(&domain);
    RecordToTuple::init_for(&mut declaration).unwrap();

    let tuple = Literal::AbstractLiteral(AbstractLiteral::Tuple(vec![
        Literal::Bool(false),
        Literal::Int(4),
    ]));
    let comparison = Expression::Eq(
        Metadata::new(),
        Moo::new(Reference::new(declaration).into()),
        Moo::new(tuple.into()),
    );

    assert!(matches!(
        run_partial_evaluator(&comparison),
        Err(RuleNotApplicable)
    ));
}

#[test]
fn packed_tuple_round_trips_integer_values() {
    let domain = Domain::tuple(vec![domain_int!(2..4), domain_int!(10..12)]);
    let state = <TuplePacked as ReprRule>::DomainLevel::init(domain).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::Tuple(vec![
        Literal::Int(3),
        Literal::Int(11),
    ]));

    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.packed, Literal::Int(4));
    assert_eq!(assignment.up(), value);
}

#[test]
fn packed_tuple_hole_constraint_is_backend_neutral() {
    let domain = Domain::tuple(vec![domain_int!(1, 3), domain_int!(5..6)]);
    let mut symbols = SymbolTable::new();
    let mut declaration = symbols.gen_find(&domain);
    let (_, constraints) = TuplePacked::init_for(&mut declaration).unwrap();

    assert_eq!(constraints.len(), 1);
    assert!(matches!(constraints[0], Expression::InDomain(..)));
}

#[test]
fn occurrence_set_round_trips_and_enforces_cardinality() {
    let domain = Domain::set(SetAttr::new_min_max_size(1, 2), domain_int!(1..3));
    let state = <SetOccurrence as ReprRule>::DomainLevel::init(domain.clone()).unwrap();
    let value =
        Literal::AbstractLiteral(AbstractLiteral::Set(vec![Literal::Int(3), Literal::Int(1)]));

    let assignment = state.down(value).unwrap();
    assert_eq!(
        assignment.up(),
        Literal::AbstractLiteral(AbstractLiteral::Set(
            vec![Literal::Int(1), Literal::Int(3),]
        ))
    );

    let mut symbols = SymbolTable::new();
    let mut declaration = symbols.gen_find(&domain);
    let (new_symbols, constraints) = SetOccurrence::init_for(&mut declaration).unwrap();
    assert_eq!(new_symbols.iter_local().count(), 3);
    assert_eq!(constraints.len(), 1);
}

#[test]
fn occurrence_set_supports_non_integer_elements() {
    let element_domain = Domain::tuple(vec![Domain::bool(), domain_int!(1..2)]);
    let domain = Domain::set(SetAttr::new_size(1), element_domain);
    let state = <SetOccurrence as ReprRule>::DomainLevel::init(domain).unwrap();
    let element = Literal::AbstractLiteral(AbstractLiteral::Tuple(vec![
        Literal::Bool(true),
        Literal::Int(2),
    ]));
    let value = Literal::AbstractLiteral(AbstractLiteral::Set(vec![element]));

    assert_eq!(state.down(value.clone()).unwrap().up(), value);
    assert!(
        state
            .occurs
            .iter()
            .all(|(key, _)| matches!(key, Literal::AbstractLiteral(AbstractLiteral::Tuple(_))))
    );
}

#[test]
fn explicit_set_round_trips_with_padding() {
    let domain = Domain::set(SetAttr::new_min_max_size(1, 3), domain_int!(1..4));
    let state = <SetExplicit as ReprRule>::DomainLevel::init(domain.clone()).unwrap();
    let value =
        Literal::AbstractLiteral(AbstractLiteral::Set(vec![Literal::Int(4), Literal::Int(2)]));

    let assignment = state.down(value).unwrap();
    assert_eq!(assignment.set_size, Some(Literal::Int(2)));
    assert_eq!(
        assignment.elems_matrix,
        Literal::from(conjure_cp::into_matrix!(vec![
            Literal::Int(2),
            Literal::Int(4),
            Literal::Int(1),
        ]))
    );
    assert_eq!(
        assignment.up(),
        Literal::AbstractLiteral(AbstractLiteral::Set(
            vec![Literal::Int(2), Literal::Int(4),]
        ))
    );

    let mut symbols = SymbolTable::new();
    let mut declaration = symbols.gen_find(&domain);
    let (new_symbols, constraints) = SetExplicit::init_for(&mut declaration).unwrap();
    assert_eq!(new_symbols.iter_local().count(), 2);
    assert_eq!(constraints.len(), 5);
    assert_eq!(
        constraints
            .iter()
            .flat_map(Uniplate::universe)
            .filter(|expr| matches!(expr, Expression::SafeIndex(..)))
            .count(),
        7
    );
}

#[test]
fn fixed_explicit_set_omits_the_cardinality_marker() {
    let domain = Domain::set(SetAttr::new_size(2), domain_int!(1..3));
    let state = <SetExplicit as ReprRule>::DomainLevel::init(domain.clone()).unwrap();
    let value =
        Literal::AbstractLiteral(AbstractLiteral::Set(vec![Literal::Int(3), Literal::Int(1)]));

    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.set_size, None);
    assert_eq!(
        assignment.up(),
        Literal::AbstractLiteral(AbstractLiteral::Set(
            vec![Literal::Int(1), Literal::Int(3),]
        ))
    );

    let mut symbols = SymbolTable::new();
    let mut declaration = symbols.gen_find(&domain);
    let (new_symbols, _) = SetExplicit::init_for(&mut declaration).unwrap();
    assert_eq!(new_symbols.iter_local().count(), 1);
    assert!(
        declaration
            .get_repr::<SetExplicit>()
            .unwrap()
            .set_size
            .is_none()
    );
}

#[test]
fn packed_set_round_trips_and_enforces_cardinality() {
    let domain = Domain::set(SetAttr::new_min_max_size(1, 2), domain_int!(1..3));
    let state = <SetPacked as ReprRule>::DomainLevel::init(domain.clone()).unwrap();
    let value =
        Literal::AbstractLiteral(AbstractLiteral::Set(vec![Literal::Int(3), Literal::Int(1)]));

    let assignment = state.down(value).unwrap();
    assert_eq!(assignment.packed, Literal::Int(5));
    assert_eq!(
        assignment.up(),
        Literal::AbstractLiteral(AbstractLiteral::Set(
            vec![Literal::Int(1), Literal::Int(3),]
        ))
    );

    let mut symbols = SymbolTable::new();
    let mut declaration = symbols.gen_find(&domain);
    let (new_symbols, constraints) = SetPacked::init_for(&mut declaration).unwrap();
    assert_eq!(new_symbols.iter_local().count(), 1);
    assert_eq!(constraints.len(), 1);

    let representation = declaration.get_repr::<SetPacked>().unwrap();
    let membership = representation.membership_expr(Expression::from(2));
    assert!(
        membership
            .universe()
            .iter()
            .any(|expr| matches!(expr, Expression::UnsafeMod(..)))
    );
    assert!(
        membership
            .universe()
            .iter()
            .all(|expr| !matches!(expr, Expression::MinionWInSet(..)))
    );

    assert_eq!(SetPacked::compactness_score(domain.clone()).unwrap(), 6);
    assert_eq!(SetOccurrence::compactness_score(domain).unwrap(), 8);
}

#[test]
fn packed_set_supports_non_integer_elements() {
    let domain = Domain::set(SetAttr::new_min_max_size(1, 2), Domain::bool());
    let state = <SetPacked as ReprRule>::DomainLevel::init(domain).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::Set(vec![Literal::Bool(true)]));

    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.packed, Literal::Int(2));
    assert_eq!(assignment.up(), value);
}

#[test]
fn explicit_mset_round_trips_and_omits_fixed_size_marker() {
    let domain = Domain::mset(MSetAttr::new(range!(3), range!(0..2)), domain_int!(1..2));
    let state = <MSetExplicit as ReprRule>::DomainLevel::init(domain).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::MSet(vec![
        Literal::Int(2),
        Literal::Int(1),
        Literal::Int(2),
    ]));

    let assignment = state.down(value).unwrap();
    assert!(assignment.mset_size.is_none());
    assert_eq!(
        assignment.up(),
        Literal::AbstractLiteral(AbstractLiteral::MSet(vec![
            Literal::Int(1),
            Literal::Int(2),
            Literal::Int(2),
        ]))
    );
}

#[test]
fn occurrence_mset_supports_non_integer_elements() {
    let domain = Domain::mset(
        MSetAttr::new(range!(0..3), range!(0..2)),
        Domain::tuple(vec![Domain::bool(), domain_int!(1..2)]),
    );
    let state = <MSetOccurrence as ReprRule>::DomainLevel::init(domain).unwrap();
    let element = Literal::AbstractLiteral(AbstractLiteral::Tuple(vec![
        Literal::Bool(true),
        Literal::Int(2),
    ]));
    let value = Literal::AbstractLiteral(AbstractLiteral::MSet(vec![element.clone(), element]));

    assert_eq!(state.down(value.clone()).unwrap().up(), value);
    assert!(
        state
            .occurs
            .iter()
            .all(|(key, _)| matches!(key, Literal::AbstractLiteral(AbstractLiteral::Tuple(_))))
    );
}

#[test]
fn packed_mset_round_trips_mixed_radix_counts() {
    let domain = Domain::mset(MSetAttr::new(range!(1..4), range!(0..2)), Domain::bool());
    let state = <MSetPacked as ReprRule>::DomainLevel::init(domain.clone()).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::MSet(vec![
        Literal::Bool(false),
        Literal::Bool(true),
        Literal::Bool(true),
    ]));

    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.packed, Literal::Int(7));
    assert_eq!(assignment.up(), value);
    assert_eq!(MSetPacked::compactness_score(domain).unwrap(), 8);
}

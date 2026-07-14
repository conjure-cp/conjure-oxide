use conjure_cp::ast::records::Field;
use conjure_cp::ast::{AbstractLiteral, Domain, Literal, Name, SetAttr, SymbolTable};
use conjure_cp::representation::{ReprAssignment, ReprDomainLevel, ReprRule};
use conjure_cp::{domain_int, range};
use conjure_cp_rules::representation::{
    MatrixToAtom, RecordToTuple, SetExplicitWithSize, SetOccurrence, TuplePacked,
};

#[test]
fn matrix_representation_initialises_and_maps_indices() {
    let domain = Domain::matrix(
        Domain::bool(),
        vec![Domain::bool(), domain_int!(1, 3, 5, 7)],
    );

    let domain_state = <MatrixToAtom as ReprRule>::DomainLevel::init(domain.clone()).unwrap();
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
    let (new_symbols, constraints) = MatrixToAtom::init_for(&mut declaration).unwrap();
    assert!(constraints.is_empty());
    assert_eq!(new_symbols.iter_local().count(), 8);
    assert!(declaration.get_repr::<MatrixToAtom>().is_some());

    let detached = declaration.detach();
    assert!(detached.get_repr::<MatrixToAtom>().is_some());
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

    let assignment = state.down(value.clone()).unwrap();
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
fn explicit_set_round_trips_with_padding() {
    let domain = Domain::set(SetAttr::new_min_max_size(1, 3), domain_int!(1..4));
    let state = <SetExplicitWithSize as ReprRule>::DomainLevel::init(domain.clone()).unwrap();
    let value =
        Literal::AbstractLiteral(AbstractLiteral::Set(vec![Literal::Int(4), Literal::Int(2)]));

    let assignment = state.down(value).unwrap();
    assert_eq!(assignment.set_size, Literal::Int(2));
    assert_eq!(
        assignment.up(),
        Literal::AbstractLiteral(AbstractLiteral::Set(
            vec![Literal::Int(2), Literal::Int(4),]
        ))
    );

    let mut symbols = SymbolTable::new();
    let mut declaration = symbols.gen_find(&domain);
    let (new_symbols, constraints) = SetExplicitWithSize::init_for(&mut declaration).unwrap();
    assert_eq!(new_symbols.iter_local().count(), 2);
    assert_eq!(constraints.len(), 2);
}

use conjure_cp::ast::records::Field;
use conjure_cp::ast::{
    AbstractLiteral, BinaryAttr, Domain, Expression, FuncAttr, GroundDomain, JectivityAttr,
    Literal, MSetAttr, Moo, Name, PartialityAttr, RelAttr, SequenceAttr, SetAttr, SymbolTable,
};
use conjure_cp::representation::{ReprAssignment, ReprDomainLevel, ReprRule};
use conjure_cp::{domain_int, range};
use conjure_cp_rules::representation::{
    FunctionAsRelation, FunctionExplicit, MSetExplicit, MSetOccurrence, MSetPacked,
    MatrixComponents, MatrixPacked, RecordComponents, RecordPacked, RelationAsSet,
    RelationOccurrence, RelationPacked, SequenceExplicit, SequencePacked, SetExplicit,
    SetOccurrence, SetPacked, TupleComponents, TuplePacked, VariantComponents, VariantPacked,
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
    assert_eq!(RecordComponents::SHORT_NAME, "components");
    assert_eq!(RecordPacked::SHORT_NAME, "packed");
    assert_eq!(VariantComponents::SHORT_NAME, "components");
    assert_eq!(VariantPacked::SHORT_NAME, "packed");
    assert_eq!(SequenceExplicit::SHORT_NAME, "explicit");
    assert_eq!(SequencePacked::SHORT_NAME, "packed");
    assert_eq!(RelationAsSet::SHORT_NAME, "as_set");
    assert_eq!(FunctionAsRelation::SHORT_NAME, "as_relation");
    assert_eq!(FunctionExplicit::SHORT_NAME, "explicit");
}

#[test]
fn variant_components_and_packed_round_trip() {
    let domain = Domain::variant(vec![
        Field {
            name: Name::user("flag"),
            value: Domain::bool(),
        },
        Field {
            name: Name::user("value"),
            value: domain_int!(2..4),
        },
    ]);
    let value = Literal::AbstractLiteral(AbstractLiteral::Variant(Moo::new(Field {
        name: Name::user("value"),
        value: Literal::Int(3),
    })));

    let components = <VariantComponents as ReprRule>::DomainLevel::init(domain.clone()).unwrap();
    let component_assignment = components.down(value.clone()).unwrap();
    assert_eq!(component_assignment.tag, Literal::Int(2));
    assert_eq!(component_assignment.fields[0], Literal::Bool(false));
    assert_eq!(component_assignment.up(), value);

    let packed = <VariantPacked as ReprRule>::DomainLevel::init(domain.clone()).unwrap();
    let packed_assignment = packed.down(value.clone()).unwrap();
    assert_eq!(packed_assignment.packed, Literal::Int(3));
    assert_eq!(packed_assignment.up(), value);
    assert_eq!(VariantPacked::compactness_score(domain).unwrap(), 5);
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
fn packed_matrix_orders_booleans_false_before_true() {
    let domain = Domain::matrix(Domain::bool(), vec![domain_int!(1..2)]);
    let state = <MatrixPacked as ReprRule>::DomainLevel::init(domain).unwrap();
    assert_eq!(
        state.values.as_ref(),
        &[Literal::Bool(false), Literal::Bool(true)]
    );
    let value = Literal::AbstractLiteral(AbstractLiteral::Matrix(
        vec![Literal::Bool(true), Literal::Bool(false)],
        Moo::new(GroundDomain::Int(vec![conjure_cp::ast::Range::Bounded(
            1, 2,
        )])),
    ));
    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.packed, Literal::Int(2));
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
fn record_components_round_trip_values_in_canonical_field_order() {
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
    let state = <RecordComponents as ReprRule>::DomainLevel::init(domain).unwrap();
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
fn packed_record_round_trips_boolean_integer_and_nested_record_fields() {
    let domain = Domain::record(vec![
        Field {
            name: Name::user("z"),
            value: Domain::record(vec![
                Field {
                    name: Name::user("flag"),
                    value: Domain::bool(),
                },
                Field {
                    name: Name::user("value"),
                    value: domain_int!(1, 3),
                },
            ]),
        },
        Field {
            name: Name::user("a"),
            value: Domain::bool(),
        },
    ]);
    let state = <RecordPacked as ReprRule>::DomainLevel::init(domain.clone()).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::Record(vec![
        Field {
            name: Name::user("z"),
            value: Literal::AbstractLiteral(AbstractLiteral::Record(vec![
                Field {
                    name: Name::user("value"),
                    value: Literal::Int(3),
                },
                Field {
                    name: Name::user("flag"),
                    value: Literal::Bool(false),
                },
            ])),
        },
        Field {
            name: Name::user("a"),
            value: Literal::Bool(false),
        },
    ]));

    let assignment = state.down(value).unwrap();
    assert_eq!(assignment.packed, Literal::Int(1));
    assert_eq!(
        assignment.up(),
        Literal::AbstractLiteral(AbstractLiteral::Record(vec![
            Field {
                name: Name::user("a"),
                value: Literal::Bool(false),
            },
            Field {
                name: Name::user("z"),
                value: Literal::AbstractLiteral(AbstractLiteral::Record(vec![
                    Field {
                        name: Name::user("flag"),
                        value: Literal::Bool(false),
                    },
                    Field {
                        name: Name::user("value"),
                        value: Literal::Int(3),
                    },
                ])),
            },
        ]))
    );
    assert_eq!(RecordPacked::compactness_score(domain).unwrap(), 8);
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
fn packed_tuple_round_trips_boolean_and_nested_values() {
    let domain = Domain::tuple(vec![
        Domain::bool(),
        Domain::tuple(vec![Domain::bool(), domain_int!(1..2)]),
    ]);
    let state = <TuplePacked as ReprRule>::DomainLevel::init(domain).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::Tuple(vec![
        Literal::Bool(false),
        Literal::AbstractLiteral(AbstractLiteral::Tuple(vec![
            Literal::Bool(true),
            Literal::Int(2),
        ])),
    ]));

    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.packed, Literal::Int(3));
    assert_eq!(assignment.up(), value);
    assert_eq!(
        state.values[0],
        vec![Literal::Bool(false), Literal::Bool(true)]
    );
}

#[test]
fn packed_tuple_uses_dense_holey_integer_digits() {
    let domain = Domain::tuple(vec![domain_int!(1, 3), domain_int!(5..6)]);
    let mut symbols = SymbolTable::new();
    let mut declaration = symbols.gen_find(&domain);
    let (_, constraints) = TuplePacked::init_for(&mut declaration).unwrap();

    assert!(constraints.is_empty());
    assert_eq!(TuplePacked::compactness_score(domain).unwrap(), 4);
}

#[test]
fn packed_tuple_supports_the_nullary_tuple() {
    let domain = Domain::tuple(vec![]);
    let state = <TuplePacked as ReprRule>::DomainLevel::init(domain).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::Tuple(vec![]));

    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.packed, Literal::Int(0));
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

fn sequence_attr(size: conjure_cp::ast::Range<i32>) -> SequenceAttr {
    SequenceAttr {
        size,
        jectivity: conjure_cp::ast::JectivityAttr::None,
        representation: None,
    }
}

#[test]
fn explicit_sequence_round_trips_fixed_length() {
    let domain = Domain::sequence(sequence_attr(range!(2)), domain_int!(1..3));
    let state = <SequenceExplicit as ReprRule>::DomainLevel::init(domain).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::Sequence(vec![
        Literal::Int(3),
        Literal::Int(1),
    ]));

    let assignment = state.down(value.clone()).unwrap();
    assert!(assignment.length.is_none());
    assert_eq!(assignment.up(), value);
}

#[test]
fn explicit_sequence_pads_variable_length_with_the_first_domain_value() {
    let domain = Domain::sequence(sequence_attr(range!(0..3)), domain_int!(1..3));
    let state = <SequenceExplicit as ReprRule>::DomainLevel::init(domain).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::Sequence(vec![Literal::Int(2)]));

    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.length, Some(Literal::Int(1)));
    let Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, _)) = &assignment.values_matrix
    else {
        panic!("expected a matrix");
    };
    // Order is preserved; inactive positions are padded with the domain's first value (not
    // sorted, unlike a set's canonical symmetry-breaking padding).
    assert_eq!(
        elems,
        &vec![Literal::Int(2), Literal::Int(1), Literal::Int(1)]
    );
    assert_eq!(assignment.up(), value);
}

#[test]
fn packed_sequence_round_trips_and_preserves_order() {
    let domain = Domain::sequence(sequence_attr(range!(0..2)), domain_int!(1..3));
    let state = <SequencePacked as ReprRule>::DomainLevel::init(domain.clone()).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::Sequence(vec![
        Literal::Int(3),
        Literal::Int(1),
    ]));

    let assignment = state.down(value.clone()).unwrap();
    let packed = assignment.packed.clone();
    assert_eq!(assignment.up(), value);

    // A different order must encode to a different packed value.
    let reversed = Literal::AbstractLiteral(AbstractLiteral::Sequence(vec![
        Literal::Int(1),
        Literal::Int(3),
    ]));
    let reversed_assignment = state.down(reversed.clone()).unwrap();
    assert_ne!(packed, reversed_assignment.packed);
    assert_eq!(reversed_assignment.up(), reversed);

    // length digit (0..=2, radix 3) * two value digits (int(1..3), radix 3 each)
    assert_eq!(
        SequencePacked::compactness_score(domain).unwrap(),
        3 * 3 * 3
    );
}

#[test]
fn packed_sequence_supports_a_fixed_length_with_no_length_digit() {
    let domain = Domain::sequence(sequence_attr(range!(2)), Domain::bool());
    let state = <SequencePacked as ReprRule>::DomainLevel::init(domain.clone()).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::Sequence(vec![
        Literal::Bool(false),
        Literal::Bool(true),
    ]));

    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.up(), value);
    assert_eq!(SequencePacked::compactness_score(domain).unwrap(), 4);
}

#[test]
fn explicit_sequence_rejects_unbounded_size() {
    let domain = Domain::sequence(
        sequence_attr(conjure_cp::ast::Range::Unbounded),
        domain_int!(1..3),
    );
    assert!(<SequenceExplicit as ReprRule>::DomainLevel::init(domain).is_err());
}

#[test]
fn relation_as_set_round_trips_binary_pairs() {
    let domain = Domain::relation(
        RelAttr {
            size: range!(2),
            binary: vec![],
        },
        vec![domain_int!(1..3), domain_int!(4..6)],
    );
    let state = <RelationAsSet as ReprRule>::DomainLevel::init(domain).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::Relation(vec![
        vec![Literal::Int(1), Literal::Int(4)],
        vec![Literal::Int(2), Literal::Int(5)],
    ]));

    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.up(), value);
}

#[test]
fn relation_as_set_round_trips_ternary_relations() {
    // Arity is no longer restricted to binary; only binary *attributes* (reflexive/symmetric/
    // etc) require exactly two columns, checked separately below.
    let domain = Domain::relation(
        RelAttr {
            size: range!(1),
            binary: vec![],
        },
        vec![domain_int!(1..3), domain_int!(1..3), domain_int!(1..3)],
    );
    let state = <RelationAsSet as ReprRule>::DomainLevel::init(domain).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::Relation(vec![vec![
        Literal::Int(1),
        Literal::Int(2),
        Literal::Int(3),
    ]]));

    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.up(), value);
}

#[test]
fn relation_as_set_rejects_binary_attributes_on_a_ternary_relation() {
    let domain = Domain::relation(
        RelAttr {
            size: range!(1),
            binary: vec![BinaryAttr::Reflexive],
        },
        vec![domain_int!(1..3), domain_int!(1..3), domain_int!(1..3)],
    );
    assert!(<RelationAsSet as ReprRule>::DomainLevel::init(domain).is_err());
}

#[test]
fn relation_as_set_rejects_binary_attributes_when_columns_differ() {
    let domain = Domain::relation(
        RelAttr {
            size: range!(1),
            binary: vec![BinaryAttr::Reflexive],
        },
        vec![domain_int!(1..3), domain_int!(4..6)],
    );
    assert!(<RelationAsSet as ReprRule>::DomainLevel::init(domain).is_err());
}

#[test]
fn relation_as_set_reflexive_builds_one_structural_constraint() {
    let domain = Domain::relation(
        RelAttr {
            size: range!(3),
            binary: vec![BinaryAttr::Reflexive],
        },
        vec![domain_int!(1..3), domain_int!(1..3)],
    );
    let mut symbols = SymbolTable::new();
    let mut declaration = symbols.gen_find(&domain);
    let (_, constraints) = RelationAsSet::init_for(&mut declaration).unwrap();
    assert_eq!(constraints.len(), 1);
}

#[test]
fn relation_occurrence_round_trips_binary_pairs() {
    let domain = Domain::relation(
        RelAttr {
            size: range!(2),
            binary: vec![],
        },
        vec![domain_int!(1..3), domain_int!(1..2)],
    );
    let state = <RelationOccurrence as ReprRule>::DomainLevel::init(domain).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::Relation(vec![
        vec![Literal::Int(1), Literal::Int(1)],
        vec![Literal::Int(2), Literal::Int(2)],
    ]));

    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.up(), value);
}

#[test]
fn relation_occurrence_round_trips_ternary_relations() {
    let domain = Domain::relation(
        RelAttr {
            size: range!(1),
            binary: vec![],
        },
        vec![domain_int!(1..2), domain_int!(1..2), Domain::bool()],
    );
    let state = <RelationOccurrence as ReprRule>::DomainLevel::init(domain).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::Relation(vec![vec![
        Literal::Int(2),
        Literal::Int(1),
        Literal::Bool(true),
    ]]));

    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.up(), value);
}

#[test]
fn relation_occurrence_rejects_a_compound_column() {
    // A set-typed column can't index a matrix; only `RelationAsSet` supports it.
    let domain = Domain::relation(
        RelAttr {
            size: range!(1),
            binary: vec![],
        },
        vec![
            domain_int!(1..2),
            Domain::set(SetAttr::new(range!(2)), domain_int!(1..3)),
        ],
    );
    assert!(<RelationOccurrence as ReprRule>::DomainLevel::init(domain).is_err());
}

#[test]
fn relation_occurrence_reflexive_and_symmetric_build_two_structural_constraints_plus_cardinality() {
    let domain = Domain::relation(
        RelAttr {
            size: conjure_cp::ast::Range::<i32>::Unbounded,
            binary: vec![BinaryAttr::Reflexive, BinaryAttr::Symmetric],
        },
        vec![domain_int!(1..3), domain_int!(1..3)],
    );
    let mut symbols = SymbolTable::new();
    let mut declaration = symbols.gen_find(&domain);
    let (_, constraints) = RelationOccurrence::init_for(&mut declaration).unwrap();
    // One cardinality range constraint (size is unbounded, so min != max) plus one formula per
    // binary attribute.
    assert_eq!(constraints.len(), 3);
}

#[test]
fn relation_occurrence_fixed_size_builds_an_equality_cardinality_constraint() {
    let domain = Domain::relation(
        RelAttr {
            size: range!(2),
            binary: vec![],
        },
        vec![domain_int!(1..3), domain_int!(1..2)],
    );
    let mut symbols = SymbolTable::new();
    let mut declaration = symbols.gen_find(&domain);
    let (_, constraints) = RelationOccurrence::init_for(&mut declaration).unwrap();
    assert_eq!(constraints.len(), 1);
}

#[test]
fn relation_packed_round_trips_binary_pairs() {
    let domain = Domain::relation(
        RelAttr {
            size: range!(2),
            binary: vec![],
        },
        vec![domain_int!(1..3), domain_int!(1..2)],
    );
    let state = <RelationPacked as ReprRule>::DomainLevel::init(domain).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::Relation(vec![
        vec![Literal::Int(1), Literal::Int(1)],
        vec![Literal::Int(2), Literal::Int(2)],
    ]));

    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.up(), value);
}

#[test]
fn relation_packed_round_trips_a_set_typed_column() {
    // More general than RelationOccurrence: any enumerable column works, not just
    // matrix-indexable ones, as long as the combined cartesian product stays small.
    let domain = Domain::relation(
        RelAttr {
            size: range!(2),
            binary: vec![],
        },
        vec![
            domain_int!(1..2),
            Domain::set(SetAttr::new(range!(2)), domain_int!(1..3)),
        ],
    );
    let state = <RelationPacked as ReprRule>::DomainLevel::init(domain).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::Relation(vec![
        vec![
            Literal::Int(1),
            Literal::AbstractLiteral(AbstractLiteral::Set(vec![Literal::Int(1), Literal::Int(2)])),
        ],
        vec![
            Literal::Int(1),
            Literal::AbstractLiteral(AbstractLiteral::Set(vec![Literal::Int(1), Literal::Int(3)])),
        ],
    ]));

    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.up(), value);
}

#[test]
fn relation_packed_rejects_a_too_large_cartesian_product() {
    // 6 columns of int(1..3) is 3^6 = 729 possible tuples, far over the 30-bit cap.
    let domain = Domain::relation(
        RelAttr {
            size: range!(2),
            binary: vec![],
        },
        vec![domain_int!(1..3); 6],
    );
    assert!(<RelationPacked as ReprRule>::DomainLevel::init(domain).is_err());
}

#[test]
fn relation_packed_reflexive_builds_a_cardinality_and_a_structural_constraint() {
    // Unlike RelationAsSet (which delegates cardinality entirely to set_decl's own eventual
    // representation), RelationPacked always emits its own cardinality constraint directly, so a
    // fixed size plus one binary attribute gives two constraints, not one.
    let domain = Domain::relation(
        RelAttr {
            size: range!(3),
            binary: vec![BinaryAttr::Reflexive],
        },
        vec![domain_int!(1..3), domain_int!(1..3)],
    );
    let mut symbols = SymbolTable::new();
    let mut declaration = symbols.gen_find(&domain);
    let (_, constraints) = RelationPacked::init_for(&mut declaration).unwrap();
    assert_eq!(constraints.len(), 2);
}

#[test]
fn relation_packed_is_more_compact_than_occurrence_for_a_bounded_size() {
    // Both represent every possible relation value, so for an *unbounded* size their compactness
    // is mathematically identical (sum_k C(n,k) == 2^n); packed only wins once the size is
    // bounded narrowly enough for the binomial sum to undercut 2^n.
    let domain = Domain::relation(
        RelAttr {
            size: range!(2),
            binary: vec![],
        },
        vec![domain_int!(1..3), domain_int!(1..3)],
    );
    assert!(
        RelationPacked::compactness_score(domain.clone()).unwrap()
            < RelationOccurrence::compactness_score(domain).unwrap()
    );
}

fn func_attr(
    size: conjure_cp::ast::Range<i32>,
    partiality: PartialityAttr,
    jectivity: JectivityAttr,
) -> FuncAttr {
    FuncAttr {
        size,
        partiality,
        jectivity,
    }
}

#[test]
fn function_as_relation_round_trips_pairs() {
    let domain = Domain::function(
        func_attr(range!(0..2), PartialityAttr::Partial, JectivityAttr::None),
        domain_int!(1..5),
        Domain::bool(),
    );
    let state = <FunctionAsRelation as ReprRule>::DomainLevel::init(domain).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::Function(vec![
        (Literal::Int(2), Literal::Bool(true)),
        (Literal::Int(4), Literal::Bool(false)),
    ]));

    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.up(), value);
}

#[test]
fn function_as_relation_total_function_builds_one_well_formedness_constraint() {
    let domain = Domain::function(
        func_attr(
            conjure_cp::ast::Range::Unbounded,
            PartialityAttr::Total,
            JectivityAttr::None,
        ),
        domain_int!(1..3),
        Domain::bool(),
    );
    let mut symbols = SymbolTable::new();
    let mut declaration = symbols.gen_find(&domain);
    let (_, constraints) = FunctionAsRelation::init_for(&mut declaration).unwrap();
    assert_eq!(constraints.len(), 1);
}

#[test]
fn function_as_relation_bijective_adds_injective_and_surjective_constraints() {
    let domain = Domain::function(
        func_attr(range!(3), PartialityAttr::Total, JectivityAttr::Bijective),
        domain_int!(1..3),
        domain_int!(1..3),
    );
    let mut symbols = SymbolTable::new();
    let mut declaration = symbols.gen_find(&domain);
    let (_, constraints) = FunctionAsRelation::init_for(&mut declaration).unwrap();
    // well-formedness + injective (pairwise) + one witness-membership pair (In + Eq) per
    // codomain value.
    assert_eq!(constraints.len(), 1 + 1 + 2 * 3);
}

#[test]
fn explicit_function_round_trips_total_function() {
    let domain = Domain::function(
        func_attr(range!(3), PartialityAttr::Total, JectivityAttr::None),
        domain_int!(1..3),
        Domain::bool(),
    );
    let state = <FunctionExplicit as ReprRule>::DomainLevel::init(domain).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::Function(vec![
        (Literal::Int(1), Literal::Bool(true)),
        (Literal::Int(2), Literal::Bool(false)),
        (Literal::Int(3), Literal::Bool(true)),
    ]));

    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.up(), value);
}

#[test]
fn explicit_function_round_trips_partial_function_and_pads_undefined_positions() {
    let domain = Domain::function(
        func_attr(range!(0..2), PartialityAttr::Partial, JectivityAttr::None),
        domain_int!(1..3),
        domain_int!(10..12),
    );
    let state = <FunctionExplicit as ReprRule>::DomainLevel::init(domain).unwrap();
    let value = Literal::AbstractLiteral(AbstractLiteral::Function(vec![(
        Literal::Int(2),
        Literal::Int(11),
    )]));

    let assignment = state.down(value.clone()).unwrap();
    assert_eq!(assignment.up(), value);
}

#[test]
fn explicit_function_total_bijective_builds_alldiff_and_surjective_constraints() {
    let domain = Domain::function(
        func_attr(range!(3), PartialityAttr::Total, JectivityAttr::Bijective),
        domain_int!(1..3),
        domain_int!(1..3),
    );
    let mut symbols = SymbolTable::new();
    let mut declaration = symbols.gen_find(&domain);
    let (_, constraints) = FunctionExplicit::init_for(&mut declaration).unwrap();
    // one allDiff (injective, total) + one surjective "or" per codomain value.
    assert_eq!(constraints.len(), 1 + 3);
}

#[test]
fn explicit_function_partial_injective_builds_guarded_pairwise_constraints() {
    let domain = Domain::function(
        func_attr(
            range!(0..3),
            PartialityAttr::Partial,
            JectivityAttr::Injective,
        ),
        domain_int!(1..3),
        domain_int!(1..3),
    );
    let mut symbols = SymbolTable::new();
    let mut declaration = symbols.gen_find(&domain);
    let (_, constraints) = FunctionExplicit::init_for(&mut declaration).unwrap();
    // one cardinality bound (maxSize 3 is not Unbounded) + guarded pairwise over 3 domain
    // positions: C(3,2) = 3 constraints.
    assert_eq!(constraints.len(), 1 + 3);
}

#[test]
fn compactness_prefers_function_as_relation_for_a_sparse_partial_function() {
    // A large domain (100 values) with at most 2 entries: the relation only needs to size
    // itself for those 2 entries, while the explicit matrix needs one atom per domain value.
    let domain = Domain::function(
        func_attr(range!(0..2), PartialityAttr::Partial, JectivityAttr::None),
        domain_int!(1..100),
        Domain::bool(),
    );
    assert!(
        FunctionAsRelation::compactness_score(domain.clone()).unwrap()
            < FunctionExplicit::compactness_score(domain).unwrap()
    );
}

#[test]
fn compactness_prefers_function_explicit_for_a_total_function() {
    // A small total function: both score similarly (entries == domain size), but explicit
    // avoids the relation's extra tuple-key overhead, so it should not score worse.
    let domain = Domain::function(
        func_attr(range!(3), PartialityAttr::Total, JectivityAttr::None),
        domain_int!(1..3),
        Domain::bool(),
    );
    assert!(
        FunctionExplicit::compactness_score(domain.clone()).unwrap()
            <= FunctionAsRelation::compactness_score(domain).unwrap()
    );
}

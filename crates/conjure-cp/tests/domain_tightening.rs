use conjure_cp::{
    ast::{GroundDomain, Name, Range},
    domain_tightening::tighten_domains_from_constraints,
    instantiate::instantiate_model,
    parse::tree_sitter::parse_essence,
    rule_engine::{resolve_rule_sets, rewrite_model},
    settings::{QuantifiedExpander, RewriteConfig, SolverFamily, set_comprehension_expander},
};
use conjure_cp_rules::representation::{MatrixComponents, SequenceExplicit};

#[allow(unused_imports)]
use conjure_cp_rules;

fn instantiated_gchq_style_model() -> conjure_cp::Model {
    let (problem, _) = parse_essence(
        r#"
language Essence 1.3
given n : int
letting ADDR be domain int(1..n)
given clues : matrix indexed by [ADDR] of sequence (maxSize n) of ADDR
find locs : matrix indexed by [ADDR] of sequence (maxSize n) of ADDR
such that
    forAll row : ADDR . |locs[row]| = |clues[row]|
"#,
    )
    .expect("parse problem");
    let (params, _) = parse_essence(
        r#"
language Essence 1.3
letting n be 5
letting clues be [
    sequence(1, 2, 3),
    sequence(1, 1, 1, 1, 1),
    sequence(1),
    sequence(1, 2),
    sequence(2, 2, 2, 2)
]
"#,
    )
    .expect("parse params");
    instantiate_model(problem, params).expect("instantiate")
}

fn sequence_sizes(name: &str, model: &conjure_cp::Model) -> Vec<Range<i32>> {
    let decl = model
        .symbols()
        .lookup_local(&Name::user(name))
        .unwrap_or_else(|| panic!("missing find `{name}`"));
    let var = decl.as_find().expect("expected a find");
    let domains = var
        .element_domains
        .as_ref()
        .unwrap_or_else(|| panic!("expected per-cell domains on `{name}`"));
    domains
        .iter()
        .map(|domain| {
            let GroundDomain::Sequence(attr, _) = domain.as_ground().unwrap() else {
                panic!("expected a sequence element domain");
            };
            attr.size.clone()
        })
        .collect()
}

#[test]
fn tightens_gchq_style_matrix_of_sequences_after_param_instantiation() {
    let mut model = instantiated_gchq_style_model();
    tighten_domains_from_constraints(&mut model);

    assert_eq!(
        sequence_sizes("locs", &model),
        vec![
            Range::Single(3),
            Range::Single(5),
            Range::Single(1),
            Range::Single(2),
            Range::Single(4)
        ]
    );
}

#[test]
fn rewrites_gchq_style_rows_as_fixed_length_sequences() {
    set_comprehension_expander(QuantifiedExpander::Native);
    let model = instantiated_gchq_style_model();
    let rule_sets = resolve_rule_sets(SolverFamily::Minion, &["Base", "Bubble"]).unwrap();
    let rewritten = rewrite_model(&model, &rule_sets, RewriteConfig::optimised()).unwrap();
    let locs = rewritten
        .symbols()
        .lookup_local(&Name::user("locs"))
        .expect("locs");
    let components = locs
        .get_repr::<MatrixComponents>()
        .expect("MatrixComponents on locs");
    let bounds: Vec<(i32, i32)> = components
        .elements
        .iter()
        .map(|elem| {
            let sequence = elem
                .get_repr::<SequenceExplicit>()
                .unwrap_or_else(|| panic!("{} should be SequenceExplicit", elem.name()));
            assert!(
                sequence.length.is_none(),
                "{} kept a length marker, so the row was not treated as fixed-length",
                elem.name()
            );
            sequence.size_bounds
        })
        .collect();
    assert_eq!(bounds, vec![(3, 3), (5, 5), (1, 1), (2, 2), (4, 4)]);
}

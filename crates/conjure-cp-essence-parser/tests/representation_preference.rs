use conjure_cp_core::ast::Name;
use conjure_cp_essence_parser::parse_essence;

/// Parse `find x: <domain>` and return `x`'s domain.
fn parse_domain(domain: &str) -> conjure_cp_core::ast::DomainPtr {
    let source = format!("language Essence 1.4\nfind x : {domain}\n");
    let (model, _) = parse_essence(&source).unwrap_or_else(|e| panic!("`{domain}`: {e:?}"));
    let symbols = model.symbols();
    let decl = symbols.lookup(&Name::user("x")).expect("x is declared");
    decl.domain().expect("x has a domain")
}

/// Every domain that can carry a preference prints it back in the syntax it was written in.
#[test]
fn preference_survives_parse_and_print() {
    for domain in [
        "int(representation order, 1..4)",
        "set (representation occurrence, size 2) of int(1..3)",
        "mset (representation occurrence, maxSize 2) of int(1..3)",
        "sequence (representation explicit, maxSize 2) of int(1..2)",
        "tuple (representation packed, int(1..3), bool)",
        "matrix (representation components) indexed by [int(1..2)] of bool",
        "record (representation packed) {a: int(1..3), b: bool}",
        "variant (representation components) {a: int(1..3), b: bool}",
        "function (representation explicit, total) int(1..2) --> int(1..2)",
        "relation (representation occurrence) of (int(1..2) * int(1..2))",
        "partition (representation occurrence) from int(1..3)",
        "permutation (representation function) of int(1..3)",
    ] {
        let parsed = parse_domain(domain);
        assert!(
            parsed.representation_preference().is_some(),
            "`{domain}` lost its preference"
        );
        let printed = parsed.to_string();
        assert_eq!(
            parse_domain(&printed),
            parsed,
            "`{domain}` printed as `{printed}`, which parses differently"
        );
    }
}

#[test]
fn int_preference_names_the_representation() {
    let domain = parse_domain("int(representation order, 1..4)");
    assert_eq!(domain.representation_preference(), Some("order"));
    assert_eq!(domain.to_string(), "int(representation order, 1..4)");
}

#[test]
fn int_preference_without_ranges_is_unbounded() {
    let domain = parse_domain("int(representation lia)");
    assert_eq!(domain.representation_preference(), Some("lia"));
    assert_eq!(
        domain.to_string(),
        "int(representation lia, -2147483647..2147483647)"
    );
}

#[test]
fn plain_int_has_no_preference() {
    let domain = parse_domain("int(1..4)");
    assert_eq!(domain.representation_preference(), None);
    assert_eq!(domain.to_string(), "int(1..4)");
}

/// A preference sits on the node it was written on and is not copied to the domains inside it.
#[test]
fn preference_is_not_inherited_by_inner_domains() {
    let domain =
        parse_domain("matrix (representation components) indexed by [int(1..2)] of int(1..3)");
    assert_eq!(domain.representation_preference(), Some("components"));
    let (inner, _) = domain.as_matrix().expect("a matrix");
    assert_eq!(inner.representation_preference(), None);
}

#[test]
fn inner_preference_is_kept_separately() {
    let domain =
        parse_domain("matrix indexed by [int(1..2)] of int(representation twos_complement, 1..3)");
    assert_eq!(domain.representation_preference(), None);
    assert!(domain.has_representation_preference());
    let (inner, _) = domain.as_matrix().expect("a matrix");
    assert_eq!(inner.representation_preference(), Some("twos_complement"));
}

#[test]
fn tuple_preference_keeps_its_members() {
    let domain = parse_domain("tuple (representation packed, int(1..3), bool)");
    assert_eq!(domain.representation_preference(), Some("packed"));
    assert_eq!(
        domain.to_string(),
        "tuple (representation packed, int(1..3), bool)"
    );
}

#[test]
fn empty_tuple_with_preference() {
    let domain = parse_domain("tuple (representation packed)");
    assert_eq!(domain.representation_preference(), Some("packed"));
}

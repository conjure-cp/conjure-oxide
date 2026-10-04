use conjure_cp_essence_parser::parse_essence;

#[test]
fn alldifferent_except_accepts_compound_exception_literals() {
    for (domain, value) in [
        ("(bool, int(0..1))", "(false, 0)"),
        (
            "record { flag : bool, value : int(0..1) }",
            "record { value = 0, flag = false }",
        ),
        ("set (maxSize 2) of int(0..1)", "{0}"),
        ("matrix indexed by [int(1..2)] of int(0..1)", "[0, 1]"),
        ("sequence (maxSize 2) of int(0..1)", "sequence(0)"),
    ] {
        let source = format!(
            "language Essence 1.3\nfind x, y : {domain}\nsuch that alldifferent_except([x,y], {value})"
        );
        parse_essence(&source).unwrap_or_else(|error| panic!("{domain}: {error:?}"));
    }
}

use conjure_cp_essence_parser::parse_essence;

#[test]
fn parses_variant_domains_literals_fields_and_active() {
    let source = r#"
language Essence 1.4
find x : variant { flag : bool, value : int(1..2) }
such that active(x, flag) -> x[flag] = true
such that x != variant { value = 2 }
"#;
    let (model, _) = parse_essence(source).unwrap();
    let printed = model.to_string();
    assert!(printed.contains("variant {flag: bool, value: int(1..2)}"));
    assert!(printed.contains("active(x, flag)"));
    assert!(printed.contains("variant{value = 2}"));
}

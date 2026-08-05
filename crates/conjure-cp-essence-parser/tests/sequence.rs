use conjure_cp_essence_parser::parse_essence;

#[test]
fn parses_sequence_domains_with_size_attributes() {
    let source = r#"
language Essence 1.4
find x : sequence (size 2) of int(1..3)
find y : sequence (maxSize 2) of int(1..3)
find z : sequence (minSize 1, maxSize 2) of int(1..3)
"#;
    let (model, _) = parse_essence(source).unwrap();
    let printed = model.to_string();
    assert!(printed.contains("sequence (size 2) of int(1..3)"));
    assert!(printed.contains("sequence (maxSize 2) of int(1..3)"));
    assert!(printed.contains("sequence (minSize 1, maxSize 2) of int(1..3)"));
}

#[test]
fn parses_sequence_literals_empty_and_populated() {
    let source = r#"
language Essence 1.4
letting A be sequence()
letting B be sequence(1, 2, 3)
"#;
    let (model, _) = parse_essence(source).unwrap();
    let printed = model.to_string();
    assert!(printed.contains("sequence()"));
    assert!(printed.contains("sequence(1,2,3)"));
}

#[test]
fn parses_sequence_domains_with_jectivity_attributes() {
    let source = r#"
language Essence 1.4
find x : sequence (size 2, injective) of int(1..3)
find y : sequence (maxSize 2, surjective) of int(1..3)
find z : sequence (size 3, bijective) of int(1..3)
"#;
    let (model, _) = parse_essence(source).unwrap();
    let printed = model.to_string();
    assert!(printed.contains("sequence (size 2, injective) of int(1..3)"));
    assert!(printed.contains("sequence (maxSize 2, surjective) of int(1..3)"));
    assert!(printed.contains("sequence (size 3, bijective) of int(1..3)"));
}

#[test]
fn parses_substring_and_subsequence_operators() {
    let source = r#"
language Essence 1.4
find x : sequence (size 2) of int(1..3)
such that (sequence(1,2) substring x)
such that (sequence(1,2) subsequence x)
"#;
    let (model, _) = parse_essence(source).unwrap();
    let printed = model.to_string();
    assert!(printed.contains("sequence(1,2) substring x"));
    assert!(printed.contains("sequence(1,2) subsequence x"));
}

use conjure_cp_essence_parser::parse_essence;

#[test]
fn parses_function_domains_with_attributes() {
    let source = r#"
language Essence 1.4
find w : function int(1..3) --> int(1..3)
find x : function (total) bool --> int(13,17)
find y : function (minSize 1, maxSize 3) int(1..3) --> int(1..3)
find z : function (size 2, injective) int(1..3) --> int(1..3)
"#;
    let (model, _) = parse_essence(source).unwrap();
    let printed = model.to_string();
    assert!(printed.contains("function  int(1..3) --> int(1..3)"));
    assert!(printed.contains("function (total) bool --> int(13, 17)"));
    assert!(printed.contains("function (minSize 1, maxSize 3) int(1..3) --> int(1..3)"));
    assert!(printed.contains("function (size 2, injective) int(1..3) --> int(1..3)"));
}

#[test]
fn parses_function_domains_with_paren_wrapped_attribute_values() {
    // Conjure's own grammar allows the attribute value to be a parenthesised integer
    // expression as well as a bare one, e.g. `minSize(3)` alongside `minSize 3`.
    let source = r#"
language Essence 1.4
find x : function (minSize(1), maxSize(3), total) int(1..3) --> int(1..3)
find y : function (size(2), bijective) int(1..3) --> int(1..3)
"#;
    let (model, _) = parse_essence(source).unwrap();
    let printed = model.to_string();
    assert!(printed.contains("function (minSize 1, maxSize 3, total) int(1..3) --> int(1..3)"));
    assert!(printed.contains("function (size 2, bijective) int(1..3) --> int(1..3)"));
}

#[test]
fn parses_function_literals_empty_and_populated() {
    let source = r#"
language Essence 1.4
letting A be function()
letting B be function(1 --> true, 2 --> false)
"#;
    let (model, _) = parse_essence(source).unwrap();
    let printed = model.to_string();
    assert!(printed.contains("function()"));
    assert!(printed.contains("function(1 --> true,2 --> false)"));
}

#[test]
fn parses_defined_operator() {
    let source = r#"
language Essence 1.4
find x : function int(1..3) --> int(1..3)
such that (defined(x) = {1,2})
"#;
    let (model, _) = parse_essence(source).unwrap();
    let printed = model.to_string();
    assert!(printed.contains("defined(x)"));
}

#[test]
fn parses_image_operator() {
    let source = r#"
language Essence 1.4
find f : function bool --> int(0..5)
find b : bool
find i : int(0..5)
such that (image(f, b) = i)
"#;
    let (model, _) = parse_essence(source).unwrap();
    let printed = model.to_string();
    assert!(printed.contains("image(f,b)"));
}

#[test]
fn parses_preimage_operator() {
    let source = r#"
language Essence 1.4
find f : function int(1..3) --> int(1..3)
such that (preImage(f, 2) = {1,2})
"#;
    let (model, _) = parse_essence(source).unwrap();
    let printed = model.to_string();
    assert!(printed.contains("preImage(f,2)"));
}

#[test]
fn parses_remaining_function_operators() {
    // NB: this deliberately avoids declaring a `relation of (...)`-typed variable, since
    // relation domains have no native tree-sitter grammar support at all yet (a pre-existing
    // gap unrelated to this function work) -- toRelation is instead exercised via a
    // function-to-function equality so it stays within what's supported.
    let source = r#"
language Essence 1.4
find x, y : function int(1..3) --> int(1..3)
find s : set of int(1..3)
letting D be domain int(1..2)
such that (range(x) = s)
such that (imageSet(x, 2) = s)
such that (inverse(x, y))
find g : function int(0..4) --> int(0..4)
such that (g = restrict(x, D))
find m : mset (maxOccur 5) of (int(1..3), int(1..3))
such that (m = toMSet(x))
such that (toRelation(x) = toRelation(y))
find t : set of (int(1..3), int(1..3))
such that (t = toSet(x))
"#;
    let (model, _) = parse_essence(source).unwrap();
    let printed = model.to_string();
    assert!(printed.contains("range(x)"));
    assert!(printed.contains("imageSet(x,2)"));
    assert!(printed.contains("inverse(x,y)"));
    assert!(printed.contains("restrict(x,D)"));
    assert!(printed.contains("toMSet(x)"));
    assert!(printed.contains("toRelation(x)"));
    assert!(printed.contains("toSet(x)"));
}

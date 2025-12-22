use conjure_cp_essence_parser::diagnostics::error_detection::semantic_errors::detect_semantic_errors;
use conjure_cp_essence_parser::diagnostics::error_detection::syntactic_errors::check_diagnostic;

#[test]
fn detects_undefined_variable() {
    let source = "find x: int(1..10)\nsuch that x = y";
    // y is undefined
    let diagnostics = detect_semantic_errors(source);

    assert_eq!(
        diagnostics.len(),
        1,
        "Expected exactly one diagnostic for undefined variable"
    );

    let diag = &diagnostics[0];

    check_diagnostic(
        diag,
        1,
        14,
        1,
        15,
        "Semantic Error: Undefined variable: 'y'",
    );
}

#[test]
fn no_errors_for_valid_code() {
    let source = "find x, y: int(1..10)\nsuch that x + y = 10";
    let diagnostics = detect_semantic_errors(source);

    // should have no diagnostics
    assert_eq!(
        diagnostics.len(),
        0,
        "Expected no diagnostics for valid code, got: {:?}",
        diagnostics
    );
}

#[test]
fn range_points_to_error_location() {
    let source = "find x: int(1..10)\nsuch that x = undefined_var";
    let diagnostics = detect_semantic_errors(source);

    assert_eq!(
        diagnostics.len(),
        1,
        "Expected exactly one diagnostic for undefined variable"
    );

    let diag = &diagnostics[0];

    check_diagnostic(
        diag,
        1,
        14,
        1,
        27,
        "Semantic Error: Undefined variable: 'undefined_var'",
    );
}

// not enforced in conjure
#[ignore]
#[test]
fn domain_start_greater_than_end() {
    let source = "find x: int(10..1)";
    let diagnostics = detect_semantic_errors(source);

    assert_eq!(
        diagnostics.len(),
        1,
        "Expected exactly one diagnostic for undefined variable"
    );

    let diag = &diagnostics[0];

    check_diagnostic(
        diag,
        0,
        12,
        0,
        17,
        "Semantic Error: Start value greater than end value in 'domain'",
    );
}

#[ignore]
#[test]
fn incorrect_type_for_equation() {
    let source = "
    letting y be false\n
    find x: int(5..10)\n
    such that 5 + y = 6";
    let diagnostics = detect_semantic_errors(source);

    assert_eq!(
        diagnostics.len(),
        1,
        "Expected exactly one diagnostic for undefined variable"
    );

    let diag = &diagnostics[0];

    check_diagnostic(
        diag,
        2,
        14,
        2,
        15,
        "Semantic Error: Incorrect type 'bool' for variable 'y', expected 'int'",
    );
}

#[ignore]
#[test]
fn dividing_over_zero() {
    let source = "find x: int(5..10)\nsuch that x/0 = 3";
    let diagnostics = detect_semantic_errors(source);

    assert_eq!(
        diagnostics.len(),
        1,
        "Expected exactly one diagnostic for undefined variable"
    );

    let diag = &diagnostics[0];

    check_diagnostic(
        diag,
        1,
        10,
        1,
        13,
        "Semantic Error: Unsafe division attempted",
    );
}

#[ignore]
#[test]
fn invalid_index() {
    let source = "letting s be (0,1,1,0)
                \nletting t be (0,0,0,1)
                \nfind a : bool such that a = (s[5] = t[1])";
    let diagnostics = detect_semantic_errors(source);

    assert_eq!(
        diagnostics.len(),
        1,
        "Expected exactly one diagnostic for undefined variable"
    );

    let diag = &diagnostics[0];

    check_diagnostic(diag, 2, 31, 2, 32, "Semantic Error: Index out of bounds");
}

#[ignore]
#[test]
fn duplicate_declaration_of_variable() {
    let source = "find x: int(1..10)\nfind x: int(2..3)";
    let diagnostics = detect_semantic_errors(source);

    assert_eq!(
        diagnostics.len(),
        1,
        "Expected exactly one diagnostic for undefined variable"
    );

    let diag = &diagnostics[0];

    check_diagnostic(
        diag,
        1,
        5,
        1,
        6,
        "Semantic Error: Redeclaration of variable 'x' which was previously defined",
    );
}

#[ignore]
#[test]
fn extra_comma_in_variable_list() {
    let source = "find x,: int(1..10)";
    let diagnostics = detect_semantic_errors(source);

    assert_eq!(
        diagnostics.len(),
        1,
        "Expected exactly one diagnostic for undefined variable"
    );

    let diag = &diagnostics[0];

    check_diagnostic(
        diag,
        0,
        6,
        0,
        7,
        "Semantic Error: Extra ',' at the end of 'variable_list'",
    );
}

use conjure_cp_essence_parser::diagnostics::error_detection::semantic_errors::detect_semantic_errors;
use conjure_cp_essence_parser::diagnostics::error_detection::syntactic_errors::check_diagnostic;

#[test]
fn detects_undefined_variable() {
    let source = "find x: int(1..10)\nsuch that x = y";
    // y is undefined
    let diagnostics = detect_semantic_errors(source);

    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic for undefined variable");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 1, 14, 1, 15, "Semantic Error: Undefined variable: 'y'");
}

#[test]
fn no_errors_for_valid_code() {
    let source = "find x, y: int(1..10)\nsuch that x + y = 10";
    let diagnostics = detect_semantic_errors(source);

    // should have no diagnostics
    assert_eq!(diagnostics.len(), 0, "Expected no diagnostics for valid code, got: {:?}", diagnostics);
}

#[test]
fn range_points_to_error_location() {
    let source = "find x: int(1..10)\nsuch that x = undefined_var";
    let diagnostics = detect_semantic_errors(source);

    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic for undefined variable");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 1, 14, 1, 27, "Semantic Error: Undefined variable: 'undefined_var'");
}

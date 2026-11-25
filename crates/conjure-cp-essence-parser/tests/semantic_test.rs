use conjure_cp_essence_parser::diagnostics::diagnostics_api::get_diagnostics;
use conjure_cp_essence_parser::diagnostics::error_detection::collect_errors::check_diagnostic;
use conjure_cp_essence_parser::util::get_tree;

#[test]
fn detects_undefined_variable() {
    let source = "find x: int(1..10)\nsuch that x = y";
    // y is undefined
    let (cst, _) = get_tree(&source).unwrap();
    let diagnostics = get_diagnostics(&source, &cst);

    assert_eq!(
        diagnostics.len(),
        1,
        "Expected exactly one diagnostic for undefined variable"
    );

    let diag = &diagnostics[0];

    check_diagnostic(diag, 1, 14, 1, 15, "The identifier 'y' is not defined");
}

#[test]
fn no_errors_for_valid_code() {
    let source = "find x, y: int(1..10)\nsuch that x + y = 10";
    let (cst, _) = get_tree(&source).unwrap();

    let diagnostics = get_diagnostics(&source, &cst);

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
    let (cst, _) = get_tree(&source).unwrap();

    let diagnostics = get_diagnostics(&source, &cst);

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
        "The identifier 'undefined_var' is not defined",
    );
}

#[test]
fn domain_start_greater_than_end_is_empty() {
    let source = "find x: int(10..1)";
    let (cst, _) = get_tree(&source).unwrap();

    let diagnostics = get_diagnostics(&source, &cst);

    assert_eq!(
        diagnostics.len(),
        0,
        "Expected reversed integer ranges to be accepted as empty domains"
    );
}

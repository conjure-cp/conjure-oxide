use conjure_cp_essence_parser::diagnostics::diagnostics_api::get_diagnostics;
use conjure_cp_essence_parser::diagnostics::error_detection::collect_errors::check_diagnostic;

#[test]
fn missing_identifier() {
    let source = "find: bool";
    let diagnostics = get_diagnostics(source);
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 0, 4, 0, 4, "Missing Variable List");
}

#[test]
fn missing_colon() {
    let source = "find x bool";
    let diagnostics = get_diagnostics(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];

    check_diagnostic(diag, 0, 6, 0, 6, "Missing :");
}

#[test]
fn missing_domain() {
    // not indented because have to avoid leading spaces for accurate character counr
    let source = "\
find x: bool
find y:
    ";

    let diagnostics = get_diagnostics(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];

    check_diagnostic(diag, 1, 7, 1, 7, "Missing Domain");
}

#[test]
fn missing_contraint() {
    // not indented because have to avoid leading spaces for accurate character counr
    let source = "\
find x: bool
such that
    ";
    let diagnostics = get_diagnostics(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];

    check_diagnostic(diag, 1, 9, 1, 9, "Missing Expression");
}

// TO-DO adapt when returning vector of errors
#[test]
fn multiple_missing_tokens() {
    // not indented because have to avoid leading spaces for accurate character counr
    let source = "\
find x: int(1..3
letting x be
    ";
    let diagnostics = get_diagnostics(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 2, "Expected two diagnostics");

    let diag1 = &diagnostics[0];
    let diag2 = &diagnostics[1];

    check_diagnostic(diag1, 0, 16, 0, 16, "Missing )");
    check_diagnostic(diag2, 1, 12, 1, 12, "Missing Expression or Domain");
}

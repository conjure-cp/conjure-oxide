use conjure_cp_essence_parser::diagnostics::diagnostics_api::get_diagnostics;
use conjure_cp_essence_parser::diagnostics::error_detection::collect_errors::check_diagnostic;

#[test]
fn invalid_top_level_statement_expression() {
    let source = " a,a,b: int(1..3)";
    let diags = get_diagnostics(source);

    assert_eq!(
        diags.len(),
        1,
        "Expected a valid top-level statement, but got 'a,a,b: int(1..3)'"
    );
    let diag = &diags[0];

    check_diagnostic(
        diag,
        0,
        0,
        0,
        17,
        "Expected a valid top-level statement, but got 'a,a,b: int(1..3)'",
    );
}

#[test]
fn malformed_find_2() {
    let source = "find >=lex,b,c: int(1..3)";
    // using >=lex operator instead of identifier
    let diagnostics = get_diagnostics(source);
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];
    check_diagnostic(
        diag,
        0,
        0,
        0,
        25,
        "Expected a find declaration statement, but got 'find >=lex,b,c: int(1..3)'",
    );
}

#[test]
fn malformed_find_3() {
    let source = "find +,a,b: int(1..3)";
    let diags = get_diagnostics(source);
    assert_eq!(diags.len(), 1, "Expected exactly one diagnostic");
    let diag = &diags[0];
    check_diagnostic(
        diag,
        0,
        0,
        0,
        21,
        "Expected a find declaration statement, but got 'find +,a,b: int(1..3)'",
    );
}

#[test]
fn unexpected_colon_used_as_identifier() {
    let source = "find :,b,c: int(1..3)";
    let diagnostics = get_diagnostics(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];

    check_diagnostic(
        diag,
        0,
        0,
        0,
        21,
        "Expected a find declaration statement, but got 'find :,b,c: int(1..3)'",
    );
}

#[test]
fn missing_colon_domain_in_find_statement_1st_line() {
    let source = "find x";
    let diagnostics = get_diagnostics(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(
        diag,
        0,
        0,
        0,
        6,
        "Expected a find declaration statement, but got 'find x'",
    );
}

#[test]
fn missing_colon_domain_in_find_statement_2nd_line() {
    let source = "find x: int(1..3)\nfind x";
    let diagnostics = get_diagnostics(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];
    check_diagnostic(
        diag,
        1,
        0,
        1,
        6,
        "Expected a find declaration statement, but got 'find x'",
    );
}

#[test]
fn unexpected_print_2nd_line() {
    let source = "find a,b,c: int(1..3)\nprint a";
    let diagnostics = get_diagnostics(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];
    check_diagnostic(
        diag,
        1,
        0,
        1,
        7,
        "Expected a valid top-level statement, but got 'print a'",
    );
}

use conjure_cp_essence_parser::diagnostics::error_detection::syntactic_errors::{
    check_diagnostic, detect_syntactic_errors, print_all_error_nodes,
};

#[test]
fn unexpected_closing_paren() {
    let source = "find x: int(1..3))";
    let diagnostics = detect_syntactic_errors(source);
    assert!(!diagnostics.is_empty(), "Expected at least one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(
        diag,
        0,
        17,
        0,
        18,
        "Unexpected token ')' at the end of 'find_statement'",
    );
}

#[test]
fn unexpected_identifier_in_range() {
    let source = "find x: int(1..3x)";
    let diagnostics = detect_syntactic_errors(source);
    assert!(!diagnostics.is_empty(), "Expected at least one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(
        diag,
        0,
        16,
        0,
        17,
        "Unexpected token 'x' inside 'int_domain'",
    );
}

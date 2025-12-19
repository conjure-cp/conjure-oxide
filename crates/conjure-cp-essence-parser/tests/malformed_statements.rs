use conjure_cp_essence_parser::diagnostics::error_detection::syntactic_errors::{
    check_diagnostic, detect_syntactic_errors, print_diagnostics,
};

#[ignore]
#[test]
fn detects_operator_as_identifier() {
    let source = "find +,b,c: int(1..3)";
    // using + operator instead of identifier
    let diagnostics = detect_syntactic_errors(source);
    print_diagnostics(&diagnostics);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 0, 0, 0, 20, "Malformed 'find' statement");
}

#[ignore]
#[test]
fn detects_complex_operator_as_identifier() {
    let source = "find >=lex,b,c: int(1..3)";
    // using >=lex operator instead of identifier
    let diagnostics = detect_syntactic_errors(source);
    print_diagnostics(&diagnostics);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 0, 0, 0, 24, "Malformed 'find_statement'");
}

#[ignore]
#[test]
fn unexpected_colon_used_as_identifier() {
    let source = "find :,b,c: int(1..3)";
    let diagnostics = detect_syntactic_errors(source);
    print_diagnostics(&diagnostics);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 0, 0, 0, 22, "Malformed 'find_statement'");
}

#[ignore]
#[test]
fn missing_colon_domain_in_find_statement() {
    let source = "find x";
    let diagnostics = detect_syntactic_errors(source);
    print_diagnostics(&diagnostics);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 0, 0, 0, 6, "Malformed 'find_statement'");
}

#[ignore]
#[test]
fn unexpected_print_keyword() {
    let source = "find a,b,c: int(1..3)\nprint a";
    let diagnostics = detect_syntactic_errors(source);
    print_diagnostics(&diagnostics);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 1, 0, 1, 7, "Malformed line");
}

#[ignore]
#[test]
fn missing_find_keyword() {
    let source = "a,b,c: int(1..3)";
    let diagnostics = detect_syntactic_errors(source);
    print_diagnostics(&diagnostics);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 0, 0, 0, 16, "Malformed line");
}

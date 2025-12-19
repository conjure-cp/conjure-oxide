use conjure_cp_essence_parser::diagnostics::error_detection::syntactic_errors::{
    check_diagnostic, detect_syntactic_errors,
};

#[test]
fn missing_identifier() {
    let source = "find: bool";
    let diagnostics = detect_syntactic_errors(source);
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 0, 4, 0, 4, "Missing 'variable_list'");
}

#[test]
fn missing_colon() {
    let source = "find x bool";
    let diagnostics = detect_syntactic_errors(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];

    check_diagnostic(diag, 0, 6, 0, 6, "Missing 'COLON'");
}

#[test]
fn missing_domain() {
    // not indented because have to avoid leading spaces for accurate character counr
    let source = "\
find x: bool
find y:
    ";

    let diagnostics = detect_syntactic_errors(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];

    check_diagnostic(diag, 1, 7, 1, 7, "Missing 'domain'");
}

#[test]
fn missing_contraint() {
    // not indented because have to avoid leading spaces for accurate character counr
    let source = "\
find x: bool
such that
    ";
    let diagnostics = detect_syntactic_errors(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];

    check_diagnostic(diag, 1, 9, 1, 9, "Missing 'bool_expr'");
}

#[test]
fn multiple_missing_tokens() {
    // not indented because have to avoid leading spaces for accurate character counr
    let source = "\
find x: int(1..3
letting x be
    ";
    let diagnostics = detect_syntactic_errors(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 2, "Expected two diagnostics");

    let diag1 = &diagnostics[0];
    let diag2 = &diagnostics[1];

    check_diagnostic(diag1, 0, 16, 0, 16, "Missing ')'");
    check_diagnostic(diag2, 1, 12, 1, 12, "Missing 'expression or domain'");
}

#[test]
fn missing_domain_in_tuple_domain() {
    let source = "find x: tuple()";
    let diagnostics = detect_syntactic_errors(source);
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(diag, 0, 14, 0, 14, "Missing 'domain'");
}

#[test]
fn missing_operator_in_comparison() {
    // Missing operator in comparison expression
    let source = "\
find x: int
such that 5 =
    ";
    let diagnostics = detect_syntactic_errors(source);
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(
        diag,
        1,
        13,
        1,
        13,
        "Missing right operand in 'comparison' expression",
    );
}

#[test]
fn missing_right_operand_in_and_expr() {
    let source = "\
find x: int
such that x /\\
";
    let diagnostics = detect_syntactic_errors(source);
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(
        diag,
        1,
        14,
        1,
        14,
        "Missing right operand in 'and' expression",
    );
}

#[test]
fn missing_period_in_domain() {
    // not indented because have to avoid leading spaces for accurate character counr
    let source = "find a: int(1.3)";

    let diagnostics = detect_syntactic_errors(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];

    check_diagnostic(diag, 0, 13, 0, 15, "Unexpected '.3' inside 'int_domain'");
}

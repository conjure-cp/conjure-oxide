use conjure_cp_essence_parser::diagnostics::error_detection::syntactic_errors::{
    check_diagnostic, detect_syntactic_errors,
};

#[test]
fn unexpected_closing_paren() {
    let source = "find x: int(1..3))";
    let diagnostics = detect_syntactic_errors(source);
    assert!(!diagnostics.is_empty(), "Expected at least one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(diag, 0, 17, 0, 18, "Unexpected ')' at the end of 'find'");
}

#[test]
fn unexpected_identifier_in_range() {
    let source = "find x: int(1..3x)";
    let diagnostics = detect_syntactic_errors(source);
    assert!(!diagnostics.is_empty(), "Expected at least one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(diag, 0, 16, 0, 17, "Unexpected 'x' inside 'int_domain'");
}

#[test]
fn unexpected_semicolon() {
    let source = "\
find x: int(1..3)
such that x = 6;
        ";
    let diagnostics = detect_syntactic_errors(source);
    assert!(!diagnostics.is_empty(), "Expected at least one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(
        diag,
        1,
        15,
        1,
        16,
        "Unexpected ';' at the end of 'such that'",
    );
}

#[test]
fn unexpected_extra_comma_in_find() {
    let source = "find x,, y: int(1..3)";
    let diagnostics = detect_syntactic_errors(source);
    assert!(!diagnostics.is_empty(), "Expected at least one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(diag, 0, 6, 0, 7, "Unexpected ',' inside 'variable_list'");
}

#[test]
fn unexpected_token_in_implication() {
    let source = "\
find x: int(1..3)
such that x -> %9
";
    let diagnostics = detect_syntactic_errors(source);
    assert!(!diagnostics.is_empty());
    let diag = &diagnostics[0];
    check_diagnostic(diag, 1, 15, 1, 16, "Unexpected '%' inside 'implication'");
}

#[test]
fn unexpected_token_in_matrix_domain() {
    let source = "find x: matrix indexed by [int, &] of int";
    let diagnostics = detect_syntactic_errors(source);
    assert!(!diagnostics.is_empty());
    let diag = &diagnostics[0];
    check_diagnostic(diag, 0, 32, 0, 33, "Unexpected '&' inside 'matrix_domain'");
}

#[test]
fn unexpected_token_in_set_literal() {
    let source = "find x: set of int\nsuch that x = {1, 2, @}";
    let diagnostics = detect_syntactic_errors(source);
    assert!(!diagnostics.is_empty());
    let diag = &diagnostics[0];

    check_diagnostic(diag, 1, 21, 1, 22, "Unexpected '@' inside 'set_literal'");
}

// Multiple unexpected tokens
// One at the end of a find statement, one inside a set
#[test]
fn multiple_unexpected_tokens() {
    let source = "\
find x: set of int;
such that x = {1, 2, @}";
    let diagnostics = detect_syntactic_errors(source);
    assert!(diagnostics.len() >= 2, "Expected at least two diagnostics");

    // First unexpected token: ';' at the end of domain
    let diag1 = &diagnostics[0];
    check_diagnostic(diag1, 0, 18, 0, 19, "Unexpected ';' at the end of 'find'");

    // Second unexpected token: '@' in set literal
    let diag2 = &diagnostics[1];
    check_diagnostic(diag2, 1, 21, 1, 22, "Unexpected '@' inside 'set_literal'");
}

#[test]
fn unexpected_x_in_all_diff() {
    let source = "\
find a : bool 
such that a = allDiff([1,2,4,1]x)";
    let diagnostics = detect_syntactic_errors(source);
    assert!(!diagnostics.is_empty(), "Expected at least one diagnostic");
    let diag = &diagnostics[0];

    check_diagnostic(
        diag,
        1,
        31,
        1,
        32,
        "Unexpected 'x' inside 'list_combining_expr_bool'",
    );
}

#[test]
fn unexpected_int_at_the_end() {
    let source = "\
find a : bool 
such that a = allDiff([1,2,4,1])8";
    let diagnostics = detect_syntactic_errors(source);
    assert!(!diagnostics.is_empty(), "Expected at least one diagnostic");
    let diag = &diagnostics[0];

    check_diagnostic(
        diag,
        1,
        32,
        1,
        33,
        "Unexpected '8' at the end of 'such that'",
    );
}

#[test]
fn unexpected_token_in_identifier() {
    let source = "find v@lue: int(1..3)";
    let diagnostics = detect_syntactic_errors(source);
    assert!(!diagnostics.is_empty(), "Expected at least one diagnostic");
    let diag = &diagnostics[0];

    check_diagnostic(
        diag,
        0,
        6,
        0,
        10,
        "Unexpected '@lue' inside 'find_statement'",
    );
}

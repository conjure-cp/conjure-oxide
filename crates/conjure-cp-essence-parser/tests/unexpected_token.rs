use conjure_cp_essence_parser::diagnostics::diagnostics_api::get_diagnostics;
use conjure_cp_essence_parser::diagnostics::error_detection::collect_errors::check_diagnostic;

#[test]
fn unexpected_closing_paren() {
    let source = "find x: int(1..3))";
    let diagnostics = get_diagnostics(source);
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(diag, 0, 17, 0, 18, "Unexpected )");
}

#[test]
fn unexpected_identifier_in_range() {
    let source = "find x: int(1..3x)";
    let diagnostics = get_diagnostics(source);
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(diag, 0, 16, 0, 17, "Unexpected x inside an Integer Domain");
}

#[test]
fn unexpected_semicolon() {
    let source = "\
find x: int(1..3)
such that x = 6;
        ";
    let diagnostics = get_diagnostics(source);
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(diag, 1, 15, 1, 16, "Unexpected ;");
}

#[test]
fn unexpected_extra_comma_in_find() {
    let source = "find x,, y: int(1..3)";
    let diagnostics = get_diagnostics(source);
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(diag, 0, 6, 0, 7, "Unexpected , inside a Variable List");
}

#[test]
fn unexpected_token_in_implication() {
    let source = "\
find x: int(1..3)
such that x -> %9
";
    let diagnostics = get_diagnostics(source);
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(diag, 1, 15, 1, 16, "Unexpected % inside an Implication");
}

#[test]
fn unexpected_token_in_matrix_domain() {
    let source = "find x: matrix indexed by [int, &] of int";
    let diagnostics = get_diagnostics(source);
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(diag, 0, 32, 0, 33, "Unexpected & inside a Matrix Domain");
}

#[test]
fn unexpected_token_in_set_literal() {
    let source = "find x: set of int\nsuch that x = {1, 2, @}";
    let diagnostics = get_diagnostics(source);
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(diag, 1, 21, 1, 22, "Unexpected @ inside a Set");
}

// Multiple unexpected tokens
// One at the end of a find statement, one inside a set
#[test]
fn multiple_unexpected_tokens() {
    let source = "\
find x: set of int;
such that x = {1, 2, @}";
    let diagnostics = get_diagnostics(source);
    assert_eq!(diagnostics.len(), 2, "Expected exactly two diagnostics");

    // First unexpected token: ';' at the end of domain
    let diag1 = &diagnostics[0];
    check_diagnostic(diag1, 0, 18, 0, 19, "Unexpected ;");

    // Second unexpected token: '@' in set literal
    let diag2 = &diagnostics[1];
    check_diagnostic(diag2, 1, 21, 1, 22, "Unexpected @ inside a Set");
}

#[test]
fn unexpected_x_in_all_diff() {
    let source = "\
find a : bool 
such that a = allDiff([1,2,4,1]x)";
    let diagnostics = get_diagnostics(source);
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];

    check_diagnostic(
        diag,
        1,
        31,
        1,
        32,
        "Unexpected x inside a List Combining Expression Bool",
    );
}

#[test]
fn unexpected_int_at_the_end() {
    let source = "\
find a : bool 
such that a = allDiff([1,2,4,1])8";
    let diagnostics = get_diagnostics(source);
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(diag, 1, 32, 1, 33, "Unexpected 8");
}

#[test]
fn unexpected_operand_at_end() {
    let source = "\
find x, a, b: int(1..3)+
";
    let diags = get_diagnostics(source);
    assert_eq!(diags.len(), 1, "Expected exactly one diagnostic");
    let diag = &diags[0];
    check_diagnostic(diag, 0, 23, 0, 24, "Unexpected +");
}

#[test]
fn unexpected_operand_middle_no_comma() {
    let source = "\
find x-, b: int(1..3)
";
    let diags = get_diagnostics(source);
    assert_eq!(diags.len(), 1, "Expected exactly one diagnostic");
    let diag = &diags[0];
    check_diagnostic(diag, 0, 6, 0, 7, "Unexpected - inside a Variable List");
}

#[test]
fn unexpected_operand_middle_comma() {
    let source = "\
find x,-, b: int(1..3)
";
    let diags = get_diagnostics(source);
    assert_eq!(diags.len(), 1, "Expected exactly one diagnostic");
    let diag = &diags[0];
    check_diagnostic(diag, 0, 6, 0, 8, "Unexpected ,- inside a Variable List");
}

#[test]
fn unexpected_token_in_identifier() {
    let source = "find v@lue: int(1..3)";
    let diagnostics = get_diagnostics(source);
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(diag, 0, 6, 0, 10, "Unexpected @lue inside a Find Statement");
}

// Temporary before better logic is developed
#[test]
fn missing_right_operand_in_and_expr() {
    let source = "\
find x: int
such that x /\\
";
    let diagnostics = get_diagnostics(source);
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(diag, 1, 12, 1, 14, "Unexpected /\\");
}

// Temporary before better logic is developed
#[test]
fn unexpected_token_in_comparison() {
    let source = "\
find x: int
such that 5 =
    ";
    let diagnostics = get_diagnostics(source);
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];
    check_diagnostic(diag, 1, 12, 1, 13, "Unexpected =");
}

#[test]
fn unexpected_token_in_domain() {
    // not indented because have to avoid leading spaces for accurate character count
    let source = "find a: int(1.3)";

    let diagnostics = get_diagnostics(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");
    let diag = &diagnostics[0];

    check_diagnostic(diag, 0, 13, 0, 15, "Unexpected .3 inside an Integer Domain");
}

// use conjure_cp_essence_parser::diagnostics::error_detection::semantic_errors::detect_semantic_errors;
// use conjure_cp_essence_parser::diagnostics::error_detection::syntactic_errors::check_diagnostic;

use conjure_cp_essence_parser::diagnostics::error_detection::collect_errors::{
    check_diagnostic, detect_errors,
};

#[test]
fn detects_keyword_as_identifier_find() {
    let source = "find find,b,c: int(1..3)";
    // using find keyword instead of identifier
    let diagnostics = detect_errors(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 0, 5, 0, 9, "Keyword 'find' used as identifier");
}

#[test]
fn detects_keyword_as_identifier_letting() {
    let source = "find letting,b,c: int(1..3)";
    // using find keyword instead of identifier
    let diagnostics = detect_errors(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 0, 5, 0, 12, "Keyword 'letting' used as identifier");
}

#[test]
fn detects_keyword_as_identifier_bool() {
    let source = "find bool: bool";
    // using find keyword instead of identifier
    let diagnostics = detect_errors(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 0, 5, 0, 9, "Keyword 'bool' used as identifier");
}

use conjure_cp_essence_parser::diagnostics_api::error_detection::semantic_errors::{detect_semantic_errors};
use conjure_cp_essence_parser::diagnostics_api::error_detection::syntactic_errors::{check_diagnostic};

#[test]
fn detects_keyword_as_identifier_find() {
    let source = "find find: int(1..3)";
    // using find keyword instead of identifier
    let diagnostics = detect_semantic_errors(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 0,5,0,9, "Keyword 'find' used as an identifier");

}

#[test]
fn detects_keyword_as_identifier_letting() {
    let source = "find letting: int(1..3)";
    // using find keyword instead of identifier
    let diagnostics = detect_semantic_errors(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 0,5,0,12, "Keyword 'letting' used as an identifier");

}

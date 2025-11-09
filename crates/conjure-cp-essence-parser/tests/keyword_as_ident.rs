use conjure_cp_essence_parser::diagnostics_api::error_detection::semantic_errors::{detect_semantic_errors};
use conjure_cp_essence_parser::diagnostics_api::error_detection::syntactic_errors::{check_diagnostic};

#[test]
fn detects_missing_identifier() {
    let source = "find find: int(1..3)";
    // using find keyword instead of identifier
    let diagnostics = detect_semantic_errors(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 0,4,0,4, "Missing 'identifier'");

}

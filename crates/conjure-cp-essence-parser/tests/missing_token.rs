use conjure_cp_essence_parser::diagnostics_api::error_detection::syntactic_errors::{detect_syntactic_errors, check_diagnostic};

#[test]
fn detects_missing_identifier() {
    let source = "find: bool";
    let diagnostics = detect_syntactic_errors(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 0,4,0,4, "Missing 'identifier'");

}
#[test]
fn detects_missing_colon() {
    let source = "find x bool";
    let diagnostics = detect_syntactic_errors(source);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 0, 6, 0, 6, "Missing ':'");

}


#[test]
fn detects_missing_domain() {
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

// with such that (missing expression)

// multiple missing tokens

// letting statememts 
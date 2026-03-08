use conjure_cp_essence_parser::diagnostics::diagnostics_api::get_diagnostics;
use conjure_cp_essence_parser::diagnostics::error_detection::collect_errors::check_diagnostic;
use tree_sitter::Tree;

#[test]
fn detects_keyword_as_identifier_find() {
    let source = "find find,b,c: int(1..3)";
    let cst: Tree = tree_sitter::Parser::new().parse(&source, None).unwrap();
    // using find keyword instead of identifier
    let diagnostics = get_diagnostics(&source, &cst);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 0, 5, 0, 9, "Keyword 'find' used as identifier");
}

#[test]
fn detects_keyword_as_identifier_letting() {
    let source = "find letting,b,c: int(1..3)";
    let cst: Tree = tree_sitter::Parser::new().parse(&source, None).unwrap();

    // using find keyword instead of identifier
    let diagnostics = get_diagnostics(&source, &cst);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 0, 5, 0, 12, "Keyword 'letting' used as identifier");
}

#[test]
fn detects_keyword_as_identifier_bool() {
    let source = "find bool: bool";
    let cst: Tree = tree_sitter::Parser::new().parse(&source, None).unwrap();

    // using find keyword instead of identifier
    let diagnostics = get_diagnostics(&source, &cst);

    // Should be exactly one diagnostic
    assert_eq!(diagnostics.len(), 1, "Expected exactly one diagnostic");

    let diag = &diagnostics[0];

    check_diagnostic(diag, 0, 5, 0, 9, "Keyword 'bool' used as identifier");
}

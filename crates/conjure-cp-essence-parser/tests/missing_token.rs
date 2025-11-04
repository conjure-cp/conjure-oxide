use conjure_cp_essence_parser::diagnostics_api::diagnostics_api::{Diagnostic, Position, Range, severity};
use conjure_cp_essence_parser::diagnostics_api::error_detection::syntactic_errors::detect_syntactic_errors;
#[test]
fn test_syntax_error_missing_vari() {
    let source = "find: bool"; // Missing variable
    let expected = vec![
        Diagnostic {
            range: Range {
                start: Position { line: 0, character: 4 },
                end: Position { line: 0, character: 4 },
            },
            severity: severity::Error,
            message: "Missing token: 'variable'".to_string(),
            source: "syntactic-error-detector",
        }
    ];
    let result = detect_syntactic_errors(source);
    assert_eq!(result, expected);
    }

[//]: # (Author: Anastasia Martinson)
[//]: # (Last Updated: 18/12/2025)

# Diagnostics API for LSP Server

## Overview

The Diagnostics API provides tree-sitter and parser–based diagnostics for Essence source files (for error underlining, syntax highlighting, and hover info). It also exposes general LSP-compatible data structures for consumption by editors and tools. Currently, the main functionality of the Diagnostics API is to parse Essence source code and report syntactic errors found by walking the CST (using tree-sitter) and report semantic errors by attempting a semantic parse and reporting parser errors. It serialises diagnostics exposing a `get_diagnostics()` function, which returns both semantic and syntactic errors as a `Vec` of `Diagnostic`s.

## Structure

```bash
crates/conjure-cp-essence-parser/src/diagnostics/
├─ mod.rs
├─ diagnostics_api.rs             # public LSP-facing structs and `get_diagnostics(source)` aggregator
└─ error_detection/
  ├─ mod.rs
  ├─ semantic_errors.rs         # AST-based semantic detection: maps parse errors to Diagnostics
  └─ syntactic_errors.rs        # tree-sitter traversal: maps erronous parse tree patterns to Diagnostics
```

### Key Functions

- `get_diagnostics(source: &str) -> Vec<Diagnostic)` serves as the main entrypoint. It aggregates `detect_semantic_errors(source)` and `detect_syntactic_errors(source)` into a single diagnostics vector, returning a combined, deduplicated-by-call vector of `Diagnostic`s. That's the function you would call for error detection and underlining.

- `detect_syntactic_errors(source: &str) -> Vec<Diagnostic)` Parses with tree-sitter and walks the CST using DFS with early retract on error/missing/zero-length nodes to avoid duplicates. More information on that in [error detection docs](error_detection/error_classification.md).

- `detect_semantic_errors(source: &str) -> Vec<Diagnostic)` Runs `parse_essence_with_context` and, on `Err(EssenceParseError)`, maps the error into a `Diagnostic` using `error_to_diagnostic`. More information on that in [error detection docs](error_detection/error_classification.md).

- Helpers (for debugging and/or testing):
  - `print_all_error_nodes(source: &str)`: prints all tree-sitter error/missing nodes with spans.
  - `print_diagnostics(diags: &[Diagnostic])`: pretty-prints diagnostics.
  - `check_diagnostic(...)`: asserts diagnostic fields in tests.

### Key Data Structures

- `Position` and `Range`: 0-based positions used to locate where in the source code the diagnostic originates from
- `Severity`: indicates the type of diagnostic, i.e., Error, Warn, Info, Hint (numeric alignment with LSP `DiagnosticSeverity`)
- `Diagnostic`:
  - `range`: `Range`
  - `severity`: `Severity`
  - `message`: human-readable description / error message
  - `source`: static string identifying which part of the API the diagnostic comes from (e.g., `syntactic-error-detector`)

 Example of serialized diagnostic:

 ```json
 {
  "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 5}},
  "severity": 1,
  "message": "some error message.",
  "source": "syntactic-error-detector"
 }
 ```

- `SymbolKind` and `DocumentSymbol`:
  - Intended for document highlighting; currently enumerates a few kinds (e.g., `Integer`, `Decimal`, `Function`, `Letting`, `Find`). To be extended in the near future.
  - `DocumentSymbol { name, detail?, kind, range, children? }`
  `DocumentSymbol` and `SymbolKind` exist to support semantic highlighting; As of now, these are scaffolded and will be extended across the Essence grammar later on.

## Direction for Use

The API can be imported via `use conjure_cp_essence_parser::diagnostics::diagnostics_api::get_diagnostics` or `use conjure_cp_essence_parser::diagnostics::diagnostics_api::*`, depending on use.

### Testing

API-specific tests are located in `crates/conjure-cp-essence-parser/tests` and can be run via

```bash
# to run all tests
cargo test -p conjure-cp-essence-parser --test

# to run with a specific test target
cargo test -p conjure-cp-essence-parser --test semantic_test
cargo test -p conjure-cp-essence-parser --test keywords_as_ident
cargo test -p conjure-cp-essence-parser --test missing_token
cargo test -p conjure-cp-essence-parser --test unexpected_token
```

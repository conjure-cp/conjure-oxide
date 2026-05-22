
## Overview

Errors found during parsing are converted into `Diagnostics` via the [Diagnostic API](https://github.com/conjure-cp/conjure-oxide/blob/main/docs/src/developers-guide/architecture/lsp/diagnostics-api.md) and then used for error underlining in the LSP.

## What the parser produces

`parse_essence_with_context_and_map` (in [`parse_model.rs`](https://github.com/conjure-cp/conjure-oxide/blob/main/crates/conjure-cp-essence-parser/src/parser/parse_model.rs)) is the shared entrypoint used by both the tree-sitter parser and the LSP.

It takes an `errors: &mut Vec<RecoverableParseError>` and:

- Builds a [`SourceMap`](https://github.com/conjure-cp/conjure-oxide/blob/main/crates/conjure-cp-essence-parser/src/diagnostics/source_map.rs), even if errors are present.
- Records **syntactic errors** first (via Tree-sitter + `detect_syntactic_errors`).
- Records **semantic errors** during the AST walk (only if no syntax errors were found; semantic error detection is suppressed when the CST contains syntax errors to avoid cascades).
- Returns `Ok((None, source_map))` if any recoverable errors were recorded; otherwise returns `Ok((Some(model), source_map))`.

## How those errors become diagnostics 

In [`collect_errors.rs`](https://github.com/conjure-cp/conjure-oxide/blob/main/crates/conjure-cp-essence-parser/src/diagnostics/error_detection/collect_errors.rs), `error_to_diagnostic` converts a `RecoverableParseError` into a `Diagnostic` struct:

- `RecoverableParseError.range` -> `Diagnostic.range` (Tree-sitter row/column → LSP-style line/character)
- `RecoverableParseError.msg` -> `Diagnostic.message`
- `Diagnostic.severity` is currently always `Error`

The function, `detect_errors(source, cst)` calls `parse_essence_with_context_and_map` and returns `Vec<Diagnostic>` using `error_to_diagnostic`. 

## How the LSP publishes diagnostics

The LSP (`crates/conjure-cp-lsp`) parses documents on `didOpen` and (debounced) `didChange` in `src/handlers/sync_event.rs`:

1. The LSP maintains a per-document cache containing the latest text, CST, AST (when available), `SourceMap`, and **the `Vec<RecoverableParseError>`** produced by parsing.
2. On open/change it calls `parse_essence_with_context_and_map(...)` and stores the returned `errors` in the cache.
3. `publish_diagnostics` converts each cached `RecoverableParseError` to a parser `Diagnostic` using `error_to_diagnostic`.
4. `convert_diagnostics` maps the parser `Diagnostic` to `tower_lsp::lsp_types::Diagnostic` and calls `client.publish_diagnostics(...)`.

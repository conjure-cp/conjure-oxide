[//]: # (Author: Anastasia Martinson)
[//]: # (Last Updated: 18/12/2025)

# Overview
The Diagnostics API provides tree-sitter and parser–based diagnostics for Essence source files (for error underlining, syntax highlighting, and hover info). It also exposes general LSP-compatible data structures for consumption by editors and tools. Currently, the main functionality of the Diagnostics API is:

- Parse Essence source code and report syntactic errors found by walking the CST (using tree-sitter).
- Attempt a semantic parse and reports parser errors (i.e., semantic errors).
- Serialises diagnostics exposing a get_diagnostics() function, which returns both semantic and syntactic errors as a `Vec` of `Diagnostic`s.

Core structures:
- `Position { line, character }`
- `Range { start: Position, end: Position }`
- `Severity` (`Error=1`, `Warn=2`, `Info=3`, `Hint=4`)
- `Diagnostic { range, severity, message, source }`

Additionally, `DocumentSymbol` and `SymbolKind` exist to support semantic highlighting; As of now, these are scaffolded and will be extended across the Essence grammar later on.

# Structure
```bash
crates/conjure-cp-essence-parser/src/diagnostics/
├─ mod.rs
├─ diagnostics_api.rs             # public LSP-facing structs and `get_diagnostics(source)` aggregator
└─ error_detection/
	 ├─ mod.rs
	 ├─ semantic_errors.rs         # AST-based semantic detection: maps parse errors to Diagnostics
	 └─ syntactic_errors.rs        # Tree-sitter traversal: maps erronous parse tree patterns to Diagnostics
```

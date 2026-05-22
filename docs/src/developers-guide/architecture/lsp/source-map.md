[//]: # "Author: Anastasia Martinson"
[//]: # "Last Updated: 22/05/2026"

# Source Map

The source map connects locations in an Essence source file to metadata discovered while parsing that file.

## Overview

The LSP uses the source map for position-based editor features:

- hover information (e.g., description of the keword at that position)
- declaration site and reference metadata
  - i.e., if we're referencing an already declared variable, we want to know where in the file it was declared (to then have the ability to visit the definition), and what type it is (to be able to highlight different types of variables differently throughout the code).
- semantic token generation (more in semantic_tokens.md)
  - used for semantic highlighting
- keeping hover and highlighting available even when the full AST cannot be built (more in recoverable_parsing.md)

The source map is built during CST-to-AST parsing in `parse_essence_with_context_and_map`, to retrieve the relevant semantic information: symbol table entries, declaration sites, domains, expression types, documentation keys, and symbol kinds.

## Implementation Locations

Source map is implemented in:

```bash
crates/conjure-cp-essence-parser/src/diagnostics/
└─ source_map.rs
```

Parser modules that populate source map spans include:

```text
crates/conjure-cp-essence-parser/src/parser/
├─ find.rs
├─ letting.rs
├─ domain.rs
├─ atom.rs
└─ expression.rs
```

## Data Structures

`SourceMap` owns all spans and keeps a range index for lookup by byte offset:

```rust
pub struct SourceMap {
    pub spans: Vec<SourceSpan>,            // stores records
    pub by_byte: RangeMap<usize, SpanId>,  // maps ranges to spanID (index into spans)
```

}

Each span records both byte offsets and Tree-sitter points:

```rust
pub struct SourceSpan {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_point: Position,
    pub end_point: Position,
    pub hover_info: Option<HoverInfo>,
}
```

`HoverInfo` stores the editor-facing metadata attached to a span:

```rust
pub struct HoverInfo {
    pub description: String,       // on-hover text
    pub doc_key: Option<String>,   // optional key into Conjure's docs/bits documentation
    pub kind: Option<SymbolKind>,  // optional semantic category, such as 'Find'
    pub ty: Option<String>,        // optional type/domain string, for example 'int(1..3)'
    pub decl_span: Option<SpanId>, // optional SpanId pointing back to the declaration span
}
```

## Populating The Map

Parser code usually adds spans with:

```rust
span_with_hover(node, source, map, hover_info)
```

This wraps `alloc_span(...)`, records the node range, stores the hover metadata, and inserts the byte range into the `RangeMap`.

For documentation-backed hovers, parser code can use:

```rust
ctx.add_span_and_doc_hover(node, doc_key, kind, ty, decl_span)
```

That stores a normalised `doc_key` in `HoverInfo`. The LSP only fetches the full documentation text when the user actually hovers that span.

Examples of current source map population:

- `find.rs` adds spans for `find`/`given` keywords and declaration variables.
- `letting.rs` adds spans for `letting` declarations.
- `domain.rs` adds spans for domain keywords and domain operators.
- `atom.rs` adds spans for variable references and constants.
- `expression.rs` adds spans for operators and function-like expressions.

For variable references, the parser looks up the declaration in the current symbol table and uses that declaration kind to set the semantic kind. This is why later occurrences of a `find` variable can be highlighted as `FindVar`, not just as a generic identifier.

## Parser Integration

The parse entry point is:

```rust
parse_essence_with_context_and_map(
    src,
    context,
    errors,
    tree,
) -> Result<(Option<Model>, SourceMap), FatalParseError>
```

The `SourceMap` is returned even when parse errors occur. This is intentional: editor features should still work for the parts of the file that were parsed successfully, even if another part of the file has a syntax or semantic error.

## Recoverable Parsing

When the CST contains Tree-sitter errors, the parser:

- records syntactic diagnostics with `detect_syntactic_errors(...)`
- suppresses following semantic errors to avoid noise and duplicate error flagging
- still walks the CST where possible
- still builds source map spans for valid subtrees
- returns `Ok((None, source_map))` if recoverable errors exist

Fatal parser errors still exist for cases where parsing cannot safely continue at all.

## LSP Cache Integration

The LSP stores the source map in its per-document cache:

```rust
pub struct CacheCont {
    pub sourcemap: Option<SourceMap>,  // here
    pub ast: Option<Model>,
    pub errors: Vec<RecoverableParseError>,
    pub cst: Option<Tree>,
    pub contents: String,
    pub version: i32,
}
```

## Accessing the Source Map

The source map lookup is:

```rust
source_map.hover_info_at_byte(hover_byte)
```

This resolves a byte offset to a `SpanId` through `by_byte`, then returns the hover metadata attached to that span.

## Byte Offsets And LSP Positions

Tree-sitter nodes use byte offsets and byte columns. Rust string slicing also uses byte offsets. The source map therefore uses bytes as its canonical internal representation.

LSP positions use line plus UTF-16 character offsets. Any LSP request must be converted before querying the map:

```text
LSP Position -> byte offset -> SourceMap lookup
```

Semantic token responses need the reverse shape:

```text
SourceMap byte range -> LSP UTF-16 line/column/length
```

This distinction matters for non-ASCII text. Byte length and LSP character length are not the same.

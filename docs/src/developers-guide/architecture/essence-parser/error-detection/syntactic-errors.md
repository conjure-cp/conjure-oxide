[//]: # (Author: Soph Morgulchik)
[//]: # (Last Updated: 21/12/2025)

# Syntactic Errors Detection

## Overview

  Syntactic errors produce an invalid **Concrete Syntax Tree (CST)** from the Tree-sitter parser ([Tree-sitter](https://tree-sitter.github.io/tree-sitter/)) when parsing Essence code. A CST produced from erroneous input may contain:

- **`ERROR` nodes**  
  Inserted when the parser cannot match part of the input to the Essence [grammar](https://github.com/conjure-cp/conjure-oxide/blob/main/crates/tree-sitter-essence/grammar.js).

- **`MISSING` nodes**  
  Inserted when the parser expects a token that is not present in the source.

If syntactic errors are present, semantic errors are not checked during parsing in [`parse_model`](https://github.com/conjure-cp/conjure-oxide/blob/main/crates/conjure-cp-essence-parser/src/parser/parse_model.rs). 


## Implementation ([syntax_errors.rs](https://github.com/conjure-cp/conjure-oxide/blob/main/crates/conjure-cp-essence-parser/src/parser/syntax_errors.rs))

```bash
crates/conjure-cp-essence-parser/src/parser
└─ syntax-errors.rs
```


#### `pub fn detect_syntactic_errors (source: &str,tree: &tree_sitter::Tree, errors: &mut Vec<RecoverableParseError>,)`

Invoked in [`parse_model`](https://github.com/conjure-cp/conjure-oxide/blob/main/crates/conjure-cp-essence-parser/src/parser/parse_model.rs). 

This function runs when the CST root node indicates an error is present. It traverses the CST, identifies and classifies syntactic errors, and appends them to the `errors` vector. Each reported issue is a [`RecoverableParseError`](https://github.com/conjure-cp/conjure-oxide/blob/main/crates/conjure-cp-essence-parser/src/errors.rs).

Traversal uses `WalkDFS::with_retract`, which is built on Tree-sitter’s [`TreeCursor`](https://docs.rs/tree-sitter/latest/tree_sitter/struct.TreeCursor.html) and an optional “retract” mode. Tree-sitter can nest errors: when a node is marked erroneous, its descendants may also be marked as errors. This can result in multiple messages for what is, effectively, a single syntax issue. Enabling retract avoids this by skipping the children of error nodes (including missing token) and collecting only top-level errors, ensuring a single diagnostic is produced per underlying error.

### Missing Token

**Detection**  
Tree-sitter does not always insert a 'MISSING' node into the CST when a token is missing from a grammar rule. To handle this robustly, missing tokens are detected using a structural check: a node whose start position equals its end position (a zero-length range) is treated as a missing token.

#### `fn classify_missing_token(node: Node, source: &str) -> RecoverableParseError`

Creates a `RecoverableParseError` for a missing token, using a context-aware message and a diagnostic range. 

**Range selection**  
The function constructs a `tree_sitter::Range` from the node’s byte offsets and row/column points, then refines it with `clamp_range_before_line_comment(&mut range, source)`. This prevents the reported span from extending into a trailing line comment. 

**Message construction**  
The message is chosen based on the missing node’s syntactic context:

- If the missing node appears within a `letting_variable_declaration`, the message is specialized to:  
  ***"Missing Expression or Domain"***.

- Otherwise, the message has the form:  
  ***"Missing `<token>`"***, where `<token>` is derived from `user_friendly_token_name(node.kind(), false)`. This helper turns internal grammar token names into user-facing text by removing underscores, substituting certain keywords with more natural wording, and adding appropriate articles.


### Unexpected Token

Unexpected tokens occur when tree-sitter fails to recognise input according to the grammar rule it is currently applying. These tokens may be valid grammar elements appearing in an unexpected context, or symbols not recognised by the grammar at all.

**Detection**  
An `ERROR` node is encountered in the CST. 

#### `fn classify_unexpected_token_error(node: Node, source_code: &str) -> RecoverableParseError`

**Range selection**  
The node’s byte offsets are clamped to the source length to avoid out-of-bounds ranges, then adjusted with `clamp_range_before_line_comment(&mut range, source)`. 

**Message construction**  
If a parent node is available (i.e., the unexpected token occurs within another construct), the message is ***"Unexpected `<token>` inside `<expression>`"***, where `<expression>` is derived via `user_friendly_token_name` to produce a human-readable label. Otherwise, the message is ***"Unexpected `<token>`"***.

The token text is extracted from `source_code` using the computed byte range. Extraction is performed on raw bytes and decoded with `String::from_utf8_lossy` to avoid panics when Tree-sitter’s byte offsets do not align with UTF-8 character boundaries (for example, when Unicode characters are present).


### Malformed Top Level Statement 

Malformed-line errors are detected when Tree-sitter produces an `ERROR` node whose shape indicates that the parser could not sensibly apply any grammar rule to the input at that point. In this situation, Tree-sitter may attempt recovery by trying alternative rules that appear to be the “best fit”. As a result, the resulting `ERROR` node can have ranges that are not reliable for diagnostics.

**Detection**  
During CST traversal, each `ERROR` node is checked with `is_malformed_line_error(&node, source)`. When it returns `true`, the line containing the error (`node.start_position().row`) is treated as malformed and a dedicated malformed-line diagnostic is emitted.

To avoid reporting duplicate diagnostics, the implementation tracks which line numbers have already been reported in `malformed_lines_reported: HashSet<usize>`. If a later `ERROR` node occurs on a line already in the set, it is skipped, ensuring at most one malformed-line message per line.

**Malformed line checks (`is_malformed_line_error`)**

- If any ancestor node kind matches  
  `find_statement`, `given_statement`, `letting_statement`, `dominance_relation`, `bool_expr`, `comparison_expr`, `arithmetic_expr`, or `atom`, the error is **not** considered a malformed-line error, returns false.
- **Position-based / range check**:
  - the error node starts at or before the first non-whitespace character on its line, or
  - the node’s span is out of range for the source (`error_node_out_of_range`).

- **Constraint-continuation exclusions**

    Even if the position/range check suggests “malformed”, the function suppresses the malformed-line classification when:
    - the line is indented (`first_non_whitespace > 0`) **and**
    - the line appears to be a constraint continuation (`is_constraint_continuation(source, row)`).

**Range selection**  
The range of the error is deliberately constrained to the bounds of the entire line (from column `0` to the line’s final character). This is done because, when no rule can apply, Tree-sitter’s recovery heuristics can produce an `ERROR` node whose byte range is misleading or extends beyond what should be highlighted. This produces a more accurate error underlining than relying on the `ERROR` node’s raw span.

**Message construction (`generate_malformed_line_message`)**  
The malformed-line message is built by taking the trimmed text of the offending line, removing anything after a `$` line comment marker, escaping any double quotes, and then classifying the line based on its first (and sometimes second) word. The final diagnostic is emitted as: **`Expected <expected>, but got '<line>'`**.

**Malformed line categories**  
Based on the line’s opening tokens, the function reports one of the following expected statement types:

- Starts with `find` → expects **a find declaration statement**
- Starts with `letting` → expects **a letting declaration statement**
- Starts with `given` → expects **a given declaration statement**
- Starts with `where` → expects **an instantiation condition**
- Starts with `minimising` or `maximising` → expects **an objective statement**
- Starts with `such that` → expects **a constraint statement**
- Starts with `such` (but not followed by `that`) → expects **a valid top-level statement**
- Any other opening token → expects **a valid top-level statement**


## How To Test

```bash
cargo test -p conjure-cp-essence-parser --test malformed_top_level
cargo test -p conjure-cp-essence-parser --test missing_token
cargo test -p conjure-cp-essence-parser --test unexpected_token
cargo test -p tests-integration --test roundtrip_tests tests_roundtrip_invalid_syntax
```

## Examples

### Example: Missing Closing Bracket

**Input**

```text
find x: int(1..2
```

**Error Message**

```text
  |
1 | find x: int(1..2
  |                 ^
Missing )
```

### Example: Missing Variable

**Input**

```text
find: bool
```

**Error Message**

```text
  |
1 | find: bool
  |     ^
Missing Variable List

```

### Example: Missing Expression/Domain

**Input**

```text
letting x be
```

**Error Message**

```text
  |
1 | letting x be
  |             ^
Missing Expression or Domain

```

### Example: Unexpected x

**Input**

```text
find x: int(1..3x)
```

**Error Message**

```text
  |
1 | find x: int(1..3x)
  |                 ^
Unexpected x inside an Integer Domain
```

### Example: Unexpected &

**Input**

```text
find x: matrix indexed by [int, &] of int
```

**Error Message**

```text
  |
1 | find x: matrix indexed by [int, &] of int
  |                                 ^
Unexpected & inside a Matrix Domain
```

### Example: Malformed letting statement

**Input**

```text
letting s be {:}
find a : int(1)
```

**Diagnostic**

```text
  |
1 | letting s be {:}
  | ^
Expected a letting declaration statement, but got 'letting s be {:}'
```

### Example: Malformed constraint statement

**Input**

```text
letting A be 3

find b: int(1..20)

such that ? b < A
```

**Diagnostic**

```text
  |
5 | such that ? b < A
  | ^
Expected a constraint statement, but got 'such that ? b < A'
```


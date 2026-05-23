[//]: # (Author: Leia McAlister-Young)
[//]: # (Last Updated: 20/05/2026)

# Overview
This page describes how error detection currently works in the `conjure-cp-essence-parser` crate at a high level.

The parser separates parser-internal failures from user-input problems, and it also separates syntax-level issues from semantic issues.

# Error Types
The parser uses two main error categories: `FatalParseError` and `RecoverableParseError`.

## Fatal Errors
Fatal errors represent parser/runtime failures where something has gone wrong in parser infrastructure or integration code, rather than in the Essence model itself.

Examples include failing to build a valid tree-sitter parse tree or hitting an internal parser error path. Fatal errors also include 'not implemented' errors. When a fatal error occurs, parsing stops and only the fatal error is returned.

## Recoverable Errors
Recoverable errors represent issues with the user input model (for example invalid syntax or semantic/type/declaration problems) where the parser can continue collecting more diagnostics. When a recoverable error is detected, it is added to the errors vector, the parser skips the rest of the erroneous branch of the parse tree, and continues parsing the rest of the input for any more errors. All recoverable errors are returned and no model is returned.

Each recoverable error stores a message and source range metadata. After all recoverable errors are found, they are enriched with file/source context for printable output.

## Packaging and Return: ParseErrorCollection
`ParseErrorCollection` is the wrapper used to return parse failures.

- `ParseErrorCollection::Fatal(...)` wraps one fatal error.
- `ParseErrorCollection::Multiple { errors: ... }` wraps many recoverable errors.

This wrapper abstracts the nature of the error(s) from the caller of the parser crate, which can simply print out the error without knowing if it is fatal or a collection of recoverable errors.

# Syntactic vs Semantic Errors
## Syntactic Errors
Syntactic errors are grammar-level failures: the source does not match grammar rules and tree-sitter recovery produces `ERROR` or `MISSING` nodes in the CST.

These are detected first by checking whether the root node has an error,which checks the entire tree. If a syntactic error exists, the CST is passed to a `detect_syntactic_errors` function which adds all syntactic errors to the errors vector. If syntactic errors exist, no semantic errors will be added to the vector (but parsing of the CST still continues for LSP source map purposes).

## Semantic Errors
Semantic errors are logically invalid constructs that may still parse into a valid CST (for example declaration/type/context errors).

These are detected during parser traversal/conversion while building model structures.

# Detection Flow (parse_model.rs)
At a high level in `parse_essence_with_context_and_map`:

1. Build or reuse a tree-sitter parse tree.
2. Check `tree.root_node().has_error()`.
3. If syntax errors exist, call `detect_syntactic_errors(...)`.
    - Semantic error reporting is then suppressed for that pass, so semantic errors are not returned when syntax errors already exist.
    - Parser traversal still proceeds to build the source-map
4. If no syntax errors exist, build a model and populate it as the parser traverses the CST
5. If a semantic error is detected, add it to the recoverable errors vector
    - Traversal and model building continues with the aim of finding any more semantic errors (but the model won't be returned)
6. Final return is either:
   - fatal error, or
   - recoverable error package, or
   - successful model.



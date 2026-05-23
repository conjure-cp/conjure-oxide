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
Semantic errors are logically invalid constructs that may still parse into valid CSTs (for example declaration/type/context errors).

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

# Syntactic Error Detection (To Be Filled)

# Semantic Error Detection
The parser does not run a separate "semantic pass" but rather semantic error checking is integrated directly into the normal parse traversal, often with if/then blocks. This is because these errors are not detectable from the CST alone and require the partially-built model for context.

When a semantic error is found, it is added to the `errors` vector of `RecoverableParseError`s stored in the shared `ParseContext` object. If the error was found in an expression, the function may return early by returning `Ok(None)` (or equivalent), which in turn causes the caller function to return early as well and so on until the top-level statement. This skips the erroneous subtree and parsing continues on the next unaffected branch. For other top-level statement types (and some expression errors), parsing may continue after an error is found, depending on the error type.

Most parser helpers return `Result<Option<T>, FatalParseError>`.

- `Err(FatalParseError)` means stop everything.
- `Ok(Some(value))` means this branch parsed successfully.
- `Ok(None)` means recoverable failure in this branch.

## Example: Duplicate Variable Declarations
Duplicate declaration checks are a good example of semantic detection during traversal. 

Example erroneous input: 

```Essence
find x, x : int
```

- In find/given declaration parsing, each identifier is parsed and collected in a `vars` vector:
    ```Rust
    let variable_name = &ctx.source_code[start..end];
    let name = Name::user(variable_name);
    ```

- If the identifier already exists in the `vars` vector (Ex. `find x, x : int`), an error is returned:
    ```Rust
    if vars.contains_key(&name) {
        ctx.errors.push(RecoverableParseError::new(
            format!(
                "Variable '{}' is already declared in this {} statement",
                variable_name,
                match symbol_kind {
                    SymbolKind::FindVar => "find",
                    SymbolKind::GivenVar => "given",
                    _ => "declaration",
                }
            ),
            Some(variable.range()),
        ));
        // don't return here, as we can still add the other variables to the symbol table
        continue;
    }
    ```

- A similar check exists for identifiers declared in previous statements. If a variable with the same name already exists in the `SymbolTable`, an error is returned, including the line number of the previous declaration.
- Once the domain is parsed, all identifiers in the `vars` vector are added to the   `SymbolTable` of the model with the domain

## Special Case: Keyword-as-Identifier Check
Most semantic checks happen during normal statement/expression parsing, but keyword-as-identifier is handled separately at the start of model parsing.

`parse_essence_with_context_and_map` calls `keyword_as_identifier(...)` before top-level statement dispatch. That function scans identifiers against a reserved-keyword set and emits recoverable semantic errors immediately.

So this check is part of semantic detection, but it is not implemented as a normal per-statement parse helper. This is because it was implemented first. It may be possible to integrate it into the normal parsing flow.

## Typechecking
Typechecking uses two context values stored in `ParseContext`:

- `typechecking_context`: the expected type of the current expression node itself.
- `inner_typechecking_context`: the expected type of elements inside a collection-like expression (for elements of a set/matrix/tuple/comprehension return value).

If a mismatch is found, a recoverable type error is recorded and that branch returns `Ok(None)`.

### When Contexts Are Set
Typechecking only occurs during expression parsing. For each top-level constraint, the parser sets `typechecking_context = Boolean` before parsing the constraint expression.

Inside expression parsing:
- Boolean-expression entry sets outer context to `Boolean`.
- Arithmetic-expression entry sets outer context to `Arithmetic`.
- Comparison parsing sets context based on comparison kind (ex. `Arithmetic` for `<`, `>`, etc., `Set` for set comparisons, etc.).
- List-combining operators such as `sum(...)` / `and(...)` set outer context to `SetOrMatrix` and inner context to `Arithmetic` or `Boolean` (respectively)
- Various special cases are implemented (ex. The type for equality expressions is inferred from the first operand and enforced on the second)

For example, while parsing the aggregate expression `min([a,b])`, the outer context would be set to `SetOrMatrix`, because the child expression must be a set of a matrix. However, the elements of the collection (`a` and `b`) must be arithmetic because of the `min` operator. Therefore, the inner context is set to `Arithmetic`.

### When Contexts Are Checked
Checks occur in multiple places:

- Expression dispatch checks obvious category mismatches (for example arithmetic expression where boolean is expected).
- Variable parsing checks declared variable domain (from `SymbolTable`) against current outer context.
- Constant parsing checks literal kind against current outer context.
- Abstract literal parsing checks the container kind itself (`set`, `matrix`, `tuple`, `record`) against outer context. Before parsing the inner elements, the inner context is promoted to the outer/main context.

### Typechecking Examples
Erroneous input (`bool_in_arith_list.essence`):

```Essence
find b: bool
find x: int(1..10)
such that sum([1, 2, b, 4]) = x
```

The typechecking error is detected as follows:
- Variables `b` and `x` are added to the `SymbolTable` with their respective domains without error
- Before parsing the constraint `sum([1, 2, b, 4]) = x`, the context is `Boolean` because it is the top level constraint. This is a comparison expression (boolean), so there is no error.
    - While parsing `sum([1, 2, b, 4])`, the outer context is set to `SetOrMatrix` because it is an aggregate expression. The inner context is set to `Arithmetic` because the operator is `sum`. 
        - `[1, 2, b, 4]` is a matrix so there is no error.
        - While parsing each element, the outer context is now `Arithmetic` (former inner context).
            - `1` and `2` are both integers so no error
                - `b` is looked up in the `SymbolTable` and the domain is `bool` so an error is added
    - The right operand `x`'s domain in the `SymbolTable` is `int(1..10)` so there is no error

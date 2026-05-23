[//]: # (Author: Leia McAlister-Young)
[//]: # (Last Updated: 20/05/2026)

# Semantic Error Detection

## Overview
The parser does not run a separate "semantic pass" but rather semantic error checking is integrated directly into the normal parse traversal, often with if/then blocks. This is because these errors are not detectable from the CST alone and require the partially-built model for context.

When a semantic error is found, it is added to the shared `errors` vector in `ParseContext`. The parser then either returns `Ok(None)` for that branch or keeps scanning sibling items if the error is local to one element. In practice, branch-level expression errors stop that subtree, while statement-level parsers can often continue so they can report more recoverable errors in the same input.

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
            - `4` is an integer so no error
    - The right operand `x`'s domain in the `SymbolTable` is `int(1..10)` so there is no error

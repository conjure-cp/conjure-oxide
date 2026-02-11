<!-- maturity: draft
authors: Edward Lowe
created: 21-12-25
---- -->

# Flatten
## Usage in Essence
With one argument, `flatten(M)` returns a one-dimensional matrix containing all the elements of the input matrix. 

With two arguments, `flatten(n,M)` the first n+1 dimensions are flattened into one dimension. 

> [WARNING!]
> `flatten(n,M)` has not yet been implemented.

## Expression variant
The `Flatten` variant of `Expression` in the AST has structure defined as
```
Flatten(Metadata, Option<Moo<Expression>>, Moo<Expression>)
```

The return type and domain of `Flatten` are a matrix of the innermost element's of the flattened matrix's return type or domain. This has not yet been implemented in the case where the dimensions to flatten is provided.

## flatten rule
The flatten rule in [crates/conjure-cp-rules/src/matrix/flatten.rs](https://github.com/conjure-cp/conjure-oxide/blob/7c0a88d8de7af41758b2020e93662db3a952ddeb/crates/conjure-cp-rules/src/matrix/flatten.rs) turns flatten expressions containing atomic matrix expressions into a flat matrix literal.

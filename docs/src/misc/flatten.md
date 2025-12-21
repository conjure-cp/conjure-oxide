<!-- maturity: draft
authors: Edward Lowe
created: 21-12-25
---- -->

# Flatten
## Usage in Essence
With one argument, `flatten(M)` returns a one-dimensional matrix containing all the elements of the input matrix. 

With two arguments, `flatten(n,M)` the first n+1 dimensions are flattened into one dimension. This feature has not yet been implemented.

## Expression variant
The `Flatten` variant of `Expression` in the AST has structure defined as
```
Flatten(Metadata, Option<Moo<Expression>>, Moo<Expression>)
```

The return type and domain of `Flatten` are a matrix of the innermost element's of the flattened matrix's return type or domain. This has not yet been implemented in the case where the dimensions to flatten is provided.

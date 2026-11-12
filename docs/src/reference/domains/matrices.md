# Matrices

## Matrix Domains

```bnf
<MatrixDomain> ::= "matrix" "indexed by" "[" list(<Domain.IntBoolEnum>) "]" "of" <Domain>
```

```essence
letting x be domain matrix indexed by [int] of int
```

## Matrix Literal

```bnf
<MatrixLiteral> ::= "[" list(<Expression>) [";" <Domain.IntBoolEnum>] "]"
```

```essence
letting x be [0,1]
```

Matrices defined without an explicit index domain will default to `int`for 1D matrices, `int, int` for 2D matrices, etc.

Alternatively, define the index domain as follows:

```essence
letting y be [0,1; int(1..2)]
```

## Matrix Operations

### `flatten`

```rs
flatten(m: matrix of T): list of T;
```

Returns a list containing the entries of a matrix with any number of dimensions.

Listed in the lexicographic order of the tuples of indices specifying each entry.

_A depth-bounded version is also available:_

```rs
flatten(n: int, m: matrix of T): matrix of T
```

Flattens the first `n+1` dimensions into one dimension.

Argument `n` must be a constant integer.

Note that `flatten(0,M) = M`.

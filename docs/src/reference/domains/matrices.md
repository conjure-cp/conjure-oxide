# Matrices

A matrix is a homogeneous n-dimensional array of values.

Unlike mathematical matrices, which are restricted to 2 dimensions and can only contain integers, Essence matrices can span any number of higher dimensions, be indexed by any primitive value (int or bool), and contain any type of value.

## Matrix Domains

The dimensionality of a matrix is specified by the 'indexed by' parameter, where the number of indexing domains is equal to the dimensionality.

A one dimensional 'list' of integers can be defined as follows:

```essence
letting x be domain matrix indexed by [int] of int
```

Similarly: a two dimensional matrix of booleans:

```essence
letting x be domain matrix indexed by [int, int] of bool
```

It's worth nothing that the indexing domains _do not have to be the same_, and _can be restricted_:

```essence
letting x be domain matrix indexed by [int(1..10), bool] of int
```

Matrices may also store other non-primitive literals (e.g. sets, sequences, other matrices).

```
letting x be domain matrix indexed by [int] of (set of bool)
```

## Matrix Literals

Matrix literals are defined in square brackets, using nested square brackets to denote higher dimensions:

```essence
letting x be [0, 1] $ 1D matrix / list
letting y be [[1, 2], [3, 4]] $ 2D matrix
```

Matrices defined without an explicit index domain will default to `int` for 1D matrices, `int, int` for 2D matrices, etc.

Alternatively, define the index domain as follows:

```essence
letting x be [0,1; int(1..2)]
letting y be [[1, 2], [3, 4]; int(1..2), int(1..2)]
```

## Matrix Operations

### `flatten`

```rs
flatten(m: matrix of T): list of T;
```

Returns a list containing the entries of a matrix with any number of dimensions.

Listed in the lexicographic order of the tuples of indices specifying each entry.

```essence
letting x be [[1, 2], [3, 4]]
letting a be flatten(x)
a = [1, 2, 3, 4]
```

_A depth-bounded version is also available:_

```rs
flatten(n: int, m: matrix of T): matrix of T
```

Flattens the first `n+1` dimensions into one dimension.

Argument `n` must be a constant integer.

Note that `flatten(0,M) = M`.

```essence
letting x be [[[1, 2], [3, 4]], [[1, 2], [3, 4]]]
letting a be flatten(1, x)
a = [[1, 2, 3, 4], [5, 6, 7, 8]]
```

## BNF

```bnf
<MatrixDomain> ::= "matrix" "indexed by" "[" list(<Domain.IntBoolEnum>) "]" "of" <Domain>
<MatrixLiteral> ::= "[" list(<Expression>) [";" <Domain.IntBoolEnum>] "]"
```

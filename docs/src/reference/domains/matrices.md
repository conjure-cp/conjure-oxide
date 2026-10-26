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

Matriced defined without an explicit index domain will default to `int`for 1D matrices, `int, int` for 2D matrices, etc.

Alternatively, define the index domain as follows:

```essence
letting y be [0,1; int(1..2)]
```

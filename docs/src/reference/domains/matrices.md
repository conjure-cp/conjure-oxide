# Matrices

## Matrix Domains

```bnf
<MatrixDomain> ::= "matrix" "indexed by" "[" list(<Domain.IntBoolEnum>) "]" "of" <Domain>
```

## Matrix Literal

```bnf
<MatrixLiteral> ::= "[" list(<Expression>) [";" <Domain.IntBoolEnum>] "]"
```

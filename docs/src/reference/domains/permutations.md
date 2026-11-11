# Permutations

## Permutation Domains

```bnf
<PermutationDomain> ::= "permutation" ["(" list(<PermutationAttribute>) ")"] "of" <Domain>
```

```essence
given a: permutation of int(1..3)
given b: permutation of int(1..3)
```

### Attributes

```bnf
<PermutationAttribute> ::= "numMoved" <Expression.Integer>
                         | "minNumMoved" <Expression.Integer>
                         | "maxNumMoved" <Expression.Integer>
```

| Attribute     | Type  | Description |
| ------------- | ----- | ----------- |
| `numMoved`    | `int` | TODO        |
| `minNumMoved` | `int` | TODO        |
| `maxNumMoved` | `int` | TODO        |

## Permutation Literals

```bnf
<PermutationLiteral> ::= "permutation" "(" [list(<PermutationLiteralCycle>)] ")"
<PermutationLiteralCycle> ::= "(" list(<Expression>) ")"
```

```essence
letting a be permutation ((1, 2, 3), (2, 1, 3), (3, 2, 1))
```

## Permutation Operations

TODO these are not well-defined.

### `permInverse`

```rs
permInverse(p: permutation of T): permutation of T
```

Returns the inverse of permutation `p`.

### `compose`

```rs
compose(p1: permutation of T, p2: permutation of T) -> permutation of T
```

Returns the composition of permutations `p1` and `p2`.

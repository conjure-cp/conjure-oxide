# Permutations

TODO what is a permutation?

## Permutation Domains

Permutations are declared using the `permutation` specifier.

```essence
given a: permutation of int(1..3)
given b: permutation (minNumMoved 1, maxNumMoved 3) of int(1..3)
```

### Permutation Attributes

| Attribute     | Type  | Description |
| ------------- | ----- | ----------- |
| `numMoved`    | `int` | TODO        |
| `minNumMoved` | `int` | TODO        |
| `maxNumMoved` | `int` | TODO        |

## Permutation Literals

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

```essence
TODO example
```

### `compose`

```rs
compose(p1: permutation of T, p2: permutation of T) -> permutation of T
```

Returns the composition of permutations `p1` and `p2`.

```essence
TODO example
```

## BNF

```bnf
<PermutationDomain> ::= "permutation" ["(" list(<PermutationAttribute>) ")"] "of" <Domain>

<PermutationAttribute> ::= "numMoved" <Expression.Integer>
                         | "minNumMoved" <Expression.Integer>
                         | "maxNumMoved" <Expression.Integer>

<PermutationLiteral> ::= "permutation" "(" [list(<PermutationLiteralCycle>)] ")"
<PermutationLiteralCycle> ::= "(" list(<Expression>) ")"
```

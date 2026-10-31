# Aggregates and Quantifiers

Aggregates and quantifiers are idiomatic ways of expressing constraints over full collections or domains.

- Aggregates return an arithmetic answer (sum / min / max of collection).
- Quantifiers return a boolean answer (for all / there exists).

## Syntax

```bnf
<AggregateExpr> ::= <AggregateType> list(<identifier>
                                       | <tuple>) ("in" <Expression.Collection>
                                                 | ":" <Domain>) ." <Expression.Boolean>

<AggregateType> ::= "forAll" | "exists" | "sum" | "min" | "max"
```

```essence
sum i: int(1..10) . 2 ** i                 $ sums the integer powers of 2 from 1 to 10
```

```
forAll j in {2, 4, 6, 9, 10} . j % 2 = 0   $ checks the set contains only even numbers (returns false here)
```

## Quantifiers

There are two quantifiers: `forAll` and `exists`.

- `forAll`: true iff the constraint holds for every element in the set, or every member of the domain.
- `exists`: true iff the constraint holds for at least one element in the set, or at least one member of the domain.

```essence
find a : bool such that a = forAll i in {0,1,2} . i=i*i $ false
find b : bool such that b = exists i : int(0..4) . i*i=i $ true
```

## Aggregates

There are three aggregates: `sum`, `min` and `max`.

- `sum`: returns the sum of values yielded by applying the expression to every element in the set, or every element of the domain.
- `min`: returns the minimum value yielded by applying the expression to every element in the set, or every element of the domain.
- `max`: returns the maximum value yielded by applying the expression to every element in the set, or every element of the domain.

# Aggregates and Quantifiers

Aggregates and quantifiers are idiomatic ways of expressing constraints over full collections or domains.

- Aggregates return an _arithmetic answer_ (sum / min / max of collection).
- Quantifiers return a _boolean answer_ (for all / there exists).

These expressions are syntactic sugar for [comprehensions](./comprehensions.md).

## Syntax

An aggregate/quantifier can act over a finite domain. The following example sums the powers of two from $2^1$ to $2^10$.

```essence
sum i: int(1..10) . 2 ** i
```

We can also iterate over a collection: the following example checks whether the set contains only even numbers:

```essence
forAll j in {2, 4, 6, 9, 10} . j % 2 = 0
$ false
```

## Quantifiers

There are two quantifiers: `forAll` and `exists`.

- `forAll`: true iff the constraint holds for every element.
- `exists`: true iff the constraint holds for at least one element.

```essence
find a : bool such that a = forAll i in {0,1,2} . i=i*i $ false
find b : bool such that b = exists i : int(0..4) . i*i=i $ true
```

## Aggregates

There are three aggregates: `sum`, `min` and `max`.

- `sum`: returns the sum of values yielded by applying the expression to every element.
- `min`: returns the minimum value yielded by applying the expression to every element.
- `max`: returns the maximum value yielded by applying the expression to every element.

## BNF

```bnf
<AggregateExpr> ::= <AggregateType> list(<identifier>
                                       | <tuple>) ("in" <Expression.Collection>
                                                 | ":" <Domain>) ." <Expression.Boolean>

<AggregateType> ::= "forAll" | "exists" | "sum" | "min" | "max"
```

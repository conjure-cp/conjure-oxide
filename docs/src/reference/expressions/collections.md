# Collections

## Collection Operations

```bnf
<SequenceComparison> ::= <Expression.Sequence> ("substring"
                                              | "subsequence") <Expression.Sequence>
```

## Indexing

Matrices, tuples, records and variants support indexing.

# Collection Operations

### `min`

```rs
min(c: collection of int): int
```

Returns the minimum integer in collection `c`.

### `max`

```rs
max(c: collection of int): int
```

Returns the maximum integer in collection `c`.

### `sum`

```rs
sum(c: collection of int): int
```

Returns the sum of all elements in collection `c`.

### `product`

```rs
product(c: collection of int): int
```

Returns the product of all elements in collection `c`.

### `and`

```rs
and(c: collection of bool): bool
```

Returns `true` if all elements in collection `c` evaluate to `true`.

### `or`

```rs
or(c: collection of bool): bool
```

Returns `true` if any element in collection `c` evaluates to `true`.

### `allDifferent`

```rs
allDifferent(c: collection of T): bool
```

Returns `true` if all elements of collection `c` are different.

### `allDifferentExcept`

```rs
allDifferentExcept(c: collection of T, e: T): bool
```

Returns `true` if all elements of collection `c` are different, ignoring elements of value `e`.

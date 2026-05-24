# Order Encoding Inequality

## Overview

```text
SATInt(a) < / > / <= / >= SATInt(b) ~> Bool
```

## What this rule does

- Normalises both operands to a shared range before comparing them.
- Uses `<` as the primitive comparison.
- Rewrites `>`, `<=`, and `>=` by swapping operands and optionally negating the result.
- Builds the comparison with a prefix scan over `not lhs_i AND rhs_i` terms.

## Prefix comparison

Order encoding's prefix-true property enables a comparison based on prefix logic. For `x < y`, we look for the first position where `x` becomes false and `y` is still true.

That idea becomes a small Tseytin chain: negate the left bit, AND it with the right bit, then OR the result into the accumulator.

# Order Encoding Division

## Overview

```text
SafeDiv(SATInt(a), SATInt(b)) ~> SATInt(c)
```

This rule closely mirrors the [Direct Encoding Division](./direct-encoding-division.md) by relying on a 2D lookup table strategy, but it requires some additional manipulations. 

The generic techniques are described in [SAT Rule Implementation Patterns](./sat-generic-patterns.md), and the current implementation lives in `crates/conjure-cp-rules/src/sat/order_int_ops.rs` (`safediv_sat_order`).

## What's new compared to Direct Encoding?

Because order encoding represents values using a cascade of `true` bits (`N >= i`), we cannot isolate a specific value simply by checking one bit. We must ensure the boundary between `true` and `false` happens exactly where the integer value would be.

This changes two main parts of the lookup table strategy:

1. **Identifying exact input values:** 
   To identify if a numerator exactly equals `i`, we must assert `(N >= i) AND NOT (N >= i+1)`. When converted to CNF via De Morgan's laws for the implication condition (i.e. `NOT condition OR consequence`), the base condition for `numerator = i` and `denominator = j` becomes a chain of `OR`s: 
   `NOT (N_i) OR (N_{i+1}) OR NOT (D_j) OR (D_{j+1})`.

2. **Building the output:** 
   When the division results in a quotient `k`, we must create a valid order-encoded bit-vector for `k`. Instead of turning on just one bit, we add a rule for every bit `m` in the output:
   - Any bit `m` up to `k` is forced to `true`.
   - Any bit `m` larger than `k` is forced to `false`.
g
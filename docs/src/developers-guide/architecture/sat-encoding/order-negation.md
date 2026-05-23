# Order Encoding Negation

## Overview

```text
-SATInt(a) ~> SATInt(b)
```

The order encoding negation rule converts a negated expression for a SATInt to a new SATInt.

The generic techniques are described in [SAT Rule Implementation Patterns](./sat-generic-patterns.md), and the exact current implementation lives in `crates/conjure-cp-rules/src/sat/order_int_ops.rs` (`neg_sat_order`).

## Rule Method
- Computes the new domain for the resultant value (the old min and old max both get negated).
- Creates a new output bitvector initially containing a single `true` bit (add padding to the front).
- Iterates backwards through the original bitvector, pushing the `tseytin_not` of each bit to the output.
- The above step reverses and negates at once.

### Why add padding to the front?
- Because order uses $\geq$ thresholds, and negation flips $\geq$ to $\leq$, for $y = -x$, we say $(y \geq k) \leftrightarrow (x \leq -k)$.
- But order encoding wants $a \geq b$, so we convert the above to $(x \leq k) \leftrightarrow ¬(x \geq k+1)$.
- The $+1$ creates an out-of-range index, so we need to account for this by inserting a `false` (which is then negated to `true`).

## Example
Consider the following example which illustrates what this rule does with a given input.

Say we have domain `D = [-3..2]`, and `x = 1`, so we want to find `y = -x = -1`. We start with `[1, 1, 1, 1, 1, 0]`.

We take the negation of the domain `D` to get the new domain `D' = [-2..3]`, and our target bitvector for `y` is `[1, 1, 0, 0, 0, 0]`.

1. Reverse bits: `[0, 1, 1, 1, 1, 1]`.
2. Insert `false` at the front: `[0, 0, 1, 1, 1, 1]`.
3. `NOT` each bit: `[1, 1, 0, 0, 0, 0]`.

We now have an order representation for `-1` as desired.

# Direct Encoding Division

## Overview

```text
SafeDiv(SATInt(a), SATInt(b)) ~> SATInt(c)
```

## What this rule does

- Calculates the exact minimum and maximum possible quotient values to tightly bound the output domain.
- Generates a new direct-encoded integer for the quotient with one boolean variable per possible value.
- Builds a 2D lookup table of clauses mapping `numerator_i AND denominator_j` to the corresponding `quotient_k`.
- Enforces that the output is a valid direct encoding by adding at-most-one constraints across all output bits.

## Lookup table strategy

Instead of mimicking a hardware division circuit, this rule relies on a **lookup table strategy** leveraging the one-hot property of direct encoding:

1. **Calculate domain bounds** - compute the smallest and largest possible quotient (`quot_min`, `quot_max`) across all combinations of the numerator and denominator bounds.
2. **Allocate output bits** - create one new boolean variable for every value in `[quot_min, quot_max]`.
3. **Map inputs to outputs** - for every possible numerator value `i` and denominator value `j`, calculate `k = floor_div(i, j)` (or `0` if `j == 0`). Then, add a CNF clause stating that if `numerator = i` and `denominator = j`, then `quotient = k`. This is encoded directly as `NOT n_i OR NOT d_j OR q_k`.
4. **Constrain output** - ensure the output quotient does not take more than one value simultaneously by adding pairwise exclusion clauses (at-most-one constraints) across all `q_k` bits.

This exploits the fact that exactly one numerator bit and exactly one denominator bit will be true, driving exactly one quotient bit to be true.
# Direct Encoding SAT Summation

## Overview

```text
Sum(SATInt(a), SATInt(b), ...) ~> SATInt(c)
```

The Direct Encoding Summation rule encodes the summation of multiple direct-encoded integers into a single direct-encoded integer.

This page stays intentionally short. The reusable technique is described in [SAT Rule Implementation Patterns](./implementation-details-overview.md), and the current source of truth is `crates/conjure-cp-rules/src/sat/direct_int_ops.rs` (`add_sat_direct`).

## What this rule does

- Normalises all operands to a shared value range.
- Builds the sum pairwise, using `tseytin_and` for each value pair and `tseytin_or` to accumulate matching terms.
- Propagates the resulting range after every addition step.
- Handles the empty-input case by returning the constant zero.

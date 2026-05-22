# Direct Encoding Absolute Value

## Overview

```text
|SATInt(a)| ~> SATInt(b)
```

The Direct Encoding Absolute Value rule encodes the absolute value operation on a direct-encoded integer into another direct-encoded integer.

This page is intentionally concise. The generic technique is described in [SAT Rule Implementation Patterns](./implementation-details-overview.md), and the current implementation lives in `crates/conjure-cp-rules/src/sat/direct_int_ops.rs` (`abs_value_sat_direct`).

## What this rule does

- Groups the one-hot input bits into buckets by `|value|`.
- Produces one output bit per bucket.
- Returns the bucket contents directly when there is only one possible contributor, which avoids unnecessary Tseytin clauses.
- Uses `false` for empty buckets.

## Bucketing by absolute value

Instead of explicitly computing absolute value through arithmetic, this rule uses a **bucketing strategy**:

1. Create one bucket for each possible output value.
2. Place every input bit into the bucket for its absolute value.
3. Emit one output bit per bucket, using OR only when a bucket has multiple contributors.

Because direct encoding is one-hot, exactly one input bit is true, which means exactly one bucket wins.

The code is the best place to check the exact bucket construction and the special-case handling for single-element buckets.

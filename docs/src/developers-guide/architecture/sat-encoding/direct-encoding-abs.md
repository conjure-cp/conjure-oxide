# Direct Encoding Absolute Value

## Overview

```text
|SATInt(a)| ~> SATInt(b)
```

The Direct Encoding Absolute Value rule encodes the absolute value operation on a direct-encoded integer into another direct-encoded integer.

### Key Characteristics

- **Input**: One `SATInt` value with direct encoding (one-hot representation).
- **Output**: A single `SATInt` with direct encoding representing its absolute value.
- **Algorithm**: Bucketing strategy that groups input bits by their absolute value.
- **Clause Growth**: Linear in domain size (minimal overhead).

## Bucketing by Absolute Value

Instead of explicitly computing absolute value through arithmetic, this rule uses a **bucketing strategy** that groups input bits by the absolute value they produce:

1. **Create buckets** - one for each possible output value.
2. **Populate buckets** - for each input value i, place its bit into the bucket corresponding to |i|.
3. **Generate output bits** - for each bucket, the output bit is the OR of all bits in that bucket.

This leverages the one-hot property: exactly one input bit is true, so exactly one bucket will have a true bit.

### Algorithm Details

```rust
let mut buckets: Vec<Vec<Expr>> = vec![Vec::new(); bucket_count];

// Group input bits by their absolute value
for value in old_min..=old_max {
    let input_bit = val_bits[(value - old_min) as usize].clone();
    let abs_value = value.abs();
    let bucket_idx = (abs_value - new_min) as usize;
    buckets[bucket_idx].push(input_bit);
}

// Generate output bits: each output position is the OR of its bucket
let mut abs_bits = Vec::with_capacity(bucket_count);
for bucket in buckets {
    let out_bit = match bucket.len() {
        0 => false_expr(),              // Empty bucket
        1 => bucket[0].clone(),         // Single element (no OR needed)
        _ => tseytin_or(&bucket, ...),  // Multiple elements: OR them
    };
    abs_bits.push(out_bit);
}
```

### Cases

1. **Range includes 0** (e.g., `[-3, 5]`):
    - Minimum output: 0 (from |0|).
    - Maximum output: 5 (from |-3| or |5|, whichever is larger).

2. **Range doesn't include 0** (e.g., `[2, 8]`):
    - Minimum output: 2 (from |2|).
    - Maximum output: 8 (from |8|).

3. **Negative range** (e.g., `[-8, -2]`):
    - Minimum output: 2 (from |-2|).
    - Maximum output: 8 (from |-8|).

## CNF Clause Generation

Output bits are generated using Tseytin OR:

```rust
let out_bit = match bucket.len() {
    0 => Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false))),
    1 => bucket[0].clone(),  // No OR needed for single element
    _ => tseytin_or(&bucket, &mut new_clauses, &mut new_symbols),
};
```

The optimization **skips Tseytin OR for single-element buckets**, reducing auxiliary variables and clauses:

- Empty bucket: Returns constant `false` (no clauses).
- Single element: Returns the element directly (no auxiliary variable).
- Multiple elements: Uses `tseytin_or` to create the disjunction.

This is efficient because buckets typically have 0, 1, or 2 elements:

- Buckets with 0 elements: Arise when the output range is sparse.
- Buckets with 1 element: When only one input value maps to that output.
- Buckets with 2 elements: When exactly two input values have the same absolute value (e.g., `-5` and `5`).

## Example: Absolute Value of Signed Integer

Given:

- Input: `X ∈ {-3, -2, -1, 0, 1, 2, 3}` (direct encoded as 7 one-hot bits).
- Output: `|X| ∈ {0, 1, 2, 3}` (direct encoded as 4 one-hot bits).

Bucket formation:

| Output Value | Input Bits that Map Here |
|---|---|
| 0 | `X₀` (from |0| = 0) |
| 1 | `X₋₁`, `X₁` (from |-1| = 1 and |1| = 1) |
| 2 | `X₋₂`, `X₂` (from |-2| = 2 and |2| = 2) |
| 3 | `X₋₃`, `X₃` (from |-3| = 3 and |3| = 3) |

Output bit generation:

```text
|X|₀ = X₀                              // Single element, no OR
|X|₁ = X₋₁ OR X₁                       // Two elements, one Tseytin OR
|X|₂ = X₋₂ OR X₂                       // Two elements, one Tseytin OR
|X|₃ = X₋₃ OR X₃                       // Two elements, one Tseytin OR
```

Clause count: 3 Tseytin ORs × 3 clauses each = 9 clauses total

## Complexity Analysis

**Time Complexity**: O(n) where n is the input range size.

**Space Complexity**: O(n) for output bits and auxiliary variables.

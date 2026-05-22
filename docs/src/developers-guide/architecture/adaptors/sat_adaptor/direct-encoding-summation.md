# Direct Encoding SAT Summation

## Overview

```text
Sum(SATInt(a), SATInt(b), ...) ~> SATInt(c)
```

The Direct Encoding Summation rule encodes the summation of multiple direct-encoded integers into a single direct-encoded integer.

### Key Characteristics

- **Input**: One or more `SATInt` values with direct encoding.
- **Output**: A single `SATInt` with direct encoding representing their sum.
- **Algorithm**: Pairwise summation using disjunctive normal form (DNF).
- **Clause Growth**: Polynomial in bit-vector sizes and domain ranges.

## Pairwise Summation with DNF

Direct encoding summation uses a **disjunctive normal form** approach that leverages the one-hot property of direct encoding.

### Core Idea

For each possible output value k, the output bit is **true if and only if** there exists **any pair of input values** i and j that sum to k:

```code
output[k] = OR over all i + j = k of (input[i] AND input[j])
```

This is computed as a series of pairwise additions, where each iteration combines an accumulator with one additional input operand.

### Algorithm Steps

```rust
// Pseudocode structure
for each additional operand:
    for each possible output value k:
        output[k] = false
        for each possible accumulator value i:
            for each possible operand value j:
                if i + j == k:
                    output[k] = output[k] OR (accum[i] AND operand[j])
```

## Operand Normalisation

Before summation, all input operands are **normalized to a common range** using `validate_direct_int_operands`:

```rust
let (mut operands, common_min, common_max) =
    validate_direct_int_operands(exprs)?;
```

This normalization:

- Identifies the global minimum and maximum across all operands.
- **Pads each operand** with leading and trailing `false` bits to match this range.
- Ensures consistency when calculating AND operations between operands.

**Example**: Summing integers with ranges `[1, 3]` and `[2, 5]`.

- Common range becomes `[1, 5]`.
- First operand padded: `[true, original_bits, false]` (where false extends the range to 5).
- Second operand padded: `[false, original_bits]` (where false extends the range to 1).

## Domain Propagation

The range of the output is determined by the possible lower and upper sums.

Lower bound = accumulator lower bound + right operand lower bound.

Upper bound = accumulator upper bound + right operand upper bound.

After each pairwise addition iteration, the accumulator's range is updated to reflect the new possible values.

## CNF Clause Generation

The OR and AND operations between bits are implemented using **Tseytin transformations**:

```rust
let and_ab = tseytin_and(&vec![a, b], &mut new_clauses, &mut new_symbols);
sum_expr = tseytin_or(&vec![sum_expr, and_ab], &mut new_clauses, &mut new_symbols);
```

Each `tseytin_and` and `tseytin_or` creates new CNF clauses and auxiliary variables, converting the DNF structure into CNF form required by SAT solvers.

## Example: Summing Two Direct-Encoded Integers

Given:

- `A ∈ {1, 2}` encoded as `[A₁, A₂]` (one-hot)
- `B ∈ {1, 2}` encoded as `[B₁, B₂]` (one-hot)

Possible sums: `{2, 3, 4}`, so output has 3 bits `[Out₂, Out₃, Out₄]`

```text
Out₂ = (A₁ AND B₁)                    // only 1+1=2
Out₃ = (A₁ AND B₂) OR (A₂ AND B₁)     // 1+2=3 or 2+1=3
Out₄ = (A₂ AND B₂)                    // only 2+2=4
```

Each OR and AND is converted to CNF via Tseytin transformation, adding auxiliary variables and clauses.

## Edge Case: Empty Sum

When the sum list is empty (degenerate case), the result is the constant `0`:

```rust
if exprs.is_empty() {
    return Ok(Reduction::pure(Expr::SATInt(
        Metadata::new(),
        SATIntEncoding::Direct,
        Moo::new(into_matrix_expr!(vec![/* true bit for value 0 */])),
        (0, 0),
    )));
}
```

## Complexity Analysis

**Time Complexity**: O(n · m²) where n is the number of operands and m is the range size

**Space Complexity**: O(n · m) for bit vectors, plus auxiliary variables from Tseytin transformations

For large domains or many operands, clause growth can be significant.

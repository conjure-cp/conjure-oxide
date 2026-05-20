# Order Encoding Inequality

## Overview

```text
SATInt(a) </>/<=/>= SATInt(b) ~> Bool
```

The Order Encoding Inequality rule encodes comparison operations (`<`, `>`, `<=`, `>=`) between two order-encoded integers into boolean CNF expressions. Order encoding represents an integer using a **prefix-true bit vector** where position i is true if the integer's value is >= i.

### Key Characteristics

- **Input**: Two `SATInt` values with order encoding (prefix-true representation)
- **Output**: A boolean CNF expression
- **Algorithm**: Prefix-OR based comparison
- **Efficiency**: Linear in bit-vector size (no quadratic term like direct encoding)

## Prefix-OR Comparison

Order encoding's prefix-true property enables a comparison based on prefix logic:

For x < y, exactly when there exists a position i where:

- x becomes false at position i (meaning x < i)
- AND y is true at position i (meaning y >= i)

In plain terms:
x < y iff there exists an i such that (not x_i and y_i)

### Algorithm Details

```rust
fn sat_order_lt(
    a_bits: Vec<Expr>,
    b_bits: Vec<Expr>,
    clauses: &mut Vec<CnfClause>,
    symbols: &mut SymbolTable,
) -> Expr {
    let mut result = false_expr();  // Start with false
    
    for (a_i, b_i) in a_bits.iter().zip(b_bits.iter()) {
        // (NOT a_i AND b_i)
        let not_a_i = tseytin_not(a_i.clone(), clauses, symbols);
        let current_term = tseytin_and(&vec![not_a_i, b_i.clone()], clauses, symbols);
        
        // Accumulate: result = result OR current_term
        result = tseytin_or(&vec![result, current_term], clauses, symbols);
    }
    result
}
```

### Why This Works for Order Encoding

In order encoding:

- Bit i is true iff the value is >= i
- When `a_i` is false and `b_i` is true: we found a position where a < b
- We OR all such positions because **any single position proves the inequality**

## Handling Multiple Comparison Operators

The rule handles all four comparison operators by:

1. **Normalizing to `<`** - transforming other operators to use `<` logic:
    - a > b → `sat_order_lt(b, a)`
    - a <= b → `NOT sat_order_lt(b, a)`
    - a >= b → `NOT sat_order_lt(a, b)`

2. **Conditionally negating** the result:

```rust
let (lhs, rhs, negate) = match expr {
    Expr::Lt(_, x, y) => (x, y, false),
    Expr::Gt(_, x, y) => (y, x, false),
    Expr::Leq(_, x, y) => (y, x, true),    // NOT (B < A)
    Expr::Geq(_, x, y) => (x, y, true),    // NOT (A < B)
    _ => return Err(RuleNotApplicable),
};

let mut output = sat_order_lt(lhs_bits, rhs_bits, ...);

if negate {
    output = tseytin_not(output, ...);
}
```

This approach reuses the core `<` logic, reducing code duplication and maintaining consistency.

## Operand Normalization

Before comparison, operands are **normalized to a common range** using `validate_order_int_operands`:

```rust
let (binding, _, _) = validate_order_int_operands(
    vec![lhs.as_ref().clone(), rhs.as_ref().clone()]
)?;
let [lhs_bits, rhs_bits] = binding.as_slice() else {
    return Err(RuleNotApplicable);
};
```

Normalization:

- Identifies the global minimum and maximum across both operands
- **Pads with leading `true` bits** for positions below an operand's minimum (since those positions are definitely true)
- **Pads with trailing `false` bits** for positions above an operand's maximum (since those positions are definitely false)

**Example**: Comparing integers with ranges `[1, 3]` and `[2, 5]`:

- Common range: `[1, 5]`
- First operand: `[true, original_bits, false]` (values < 1 are implicitly >= 1)
- Second operand: `[false, original_bits]` (values < 2 are not >= 1)

## CNF Clause Generation

The prefix-OR logic is built using Tseytin transformations:

```rust
let not_a_i = tseytin_not(a_i.clone(), clauses, symbols);
let current_term = tseytin_and(&vec![not_a_i, b_i.clone()], clauses, symbols);
result = tseytin_or(&vec![result, current_term], clauses, symbols);
```

Each operation creates auxiliary variables and CNF clauses:

- `tseytin_not` adds one clause: x implies not y, and not y implies x
- `tseytin_and` adds three clauses encoding y implies x1 and x2
- `tseytin_or` adds three clauses encoding y is implied by x1 or x2

## Key Design Decisions

### Why Normalize Operands?

Normalization ensures zip iteration is safe:

```rust
for (a_i, b_i) in a_bits.iter().zip(b_bits.iter()) { ... }
```

Without matching ranges, zip would terminate at the shorter vector, missing critical comparison positions.

## Example: Comparing Two Order-Encoded Integers

Given:

- `A ∈ {2, 3, 4}` order encoded as `[A₂, A₃, A₄]` where bit i represents >= i
  - Value 2: `[true, false, false]` (2 ≥ 2, but not 2 ≥ 3)
  - Value 3: `[true, true, false]` (3 ≥ 2 and 3 ≥ 3)
  - Value 4: `[true, true, true]` (4 ≥ 2, 3, 4)

- `B ∈ {1, 3, 4}` order encoded as `[B₁, B₂, B₃, B₄]`
  - Value 1: `[true, false, false, false]`
  - Value 3: `[true, true, true, false]`
  - Value 4: `[true, true, true, true]`

Computing A < B requires normalized ranges [1, 4]:

```text
A normalized: [true, A₂, A₃, A₄]    (A₁ is implicitly true)
B normalized: [B₁, B₂, B₃, B₄]     (no change needed)

A < B iff:
    (NOT A₁ AND B₁) OR
    (NOT A₂ AND B₂) OR
    (NOT A₃ AND B₃) OR
    (NOT A₄ AND B₄)
```

Each OR and AND is converted to CNF via Tseytin, totalling approximately 12 clauses.

## Complexity Analysis

**Time Complexity**: O(n) where n is the maximum bit-vector size

**Space Complexity**: O(n) auxiliary variables from O(n) Tseytin operations

**Clause Count**: Approximately 3n CNF clauses for one comparison.

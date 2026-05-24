# Creating SAT Transformation Rules

## Overview

Most SAT rules in conjure-oxide follow the same small set of implementation patterns. The details change from rule to rule, but the structure is usually the same: validate the input, normalise the operands, build a compact Boolean construction, convert it to CNF with Tseytin helpers, and propagate the output domain if the result is an integer.

## General Workflow

Regardless of which encoding type is being used, a SAT transformation rule should follow these steps:

1. **Input validation** - Standard for any rule; check that the input expression is the target for this rule and that sub-components are valid.
2. **Extract raw data** - Extract operand bit vectors and ranges (especially for integer operations).
3. **Normalise operands** - Most integer rules should pad bit-vectors to a shared range so later zips or index lookups are safe.
4. **Create the new expression with CNF clauses** - Use dedicated `tseytin_...` functions to construct boolean expressions. These functions directly generate CNF clauses and manage auxiliary variables, significantly reducing the rule applications needed for solver-ready input. For the logic behind boolean-to-CNF conversion, refer to the next chapter: [Booleans](booleans.md).
5. **Domain propagation (Integers only)** - Update the range of the returned `SATInt` to reflect the new interval. The bit-width of the `SATInt` should also be updated for the new range.
6. **Return the result** - Use `Reduction::cnf(..)` to return the created expression along with the new CNF clauses and symbol table.

## Example - negation of log integers

```rust
/// Converts negation of a SATInt to a SATInt
///
/// ```text
/// -SATInt(a) ~> SATInt(b)
///
/// ```
#[register_rule("SAT_Log", 4100, [Neg])]
fn cnf_int_neg(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Neg(_, expr) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::SATInt(_, _, _, (min, max)) = expr.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let binding = validate_log_int_operands(vec![expr.as_ref().clone()], None)?;
    let [bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let mut new_clauses = vec![];
    let mut new_symbols = symbols.clone();

    let result = tseytin_negate(bits, bits.len(), &mut new_clauses, &mut new_symbols);

    Ok(Reduction::cnf(
        Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(into_matrix_expr!(result)),
            (-max, -min),
        ),
        new_clauses,
        new_symbols,
    ))
}
```

### Assigning Rule Priorities

Following the general conventions for rule priorities, SAT specific rules should have priorities at the 4000 level. As rules are applied in priority order (high to low), the following convention has been developed that roughly follows the expected order of rule application.

| Rule Type                           | Priority | Description |
|-------------------------------------|----------|---------|
| Integer Decision Variable -> SATInt | 4800     | These are the rules that create the SATInt expressions that the other rules can understand. |
| SATInt -> SATInt                    | 4700     | These are rules that represent standard integer-to-integer operations; summation, abs, pow, etc. |
| SATInt -> Boolean                   | 4600     | These are rules that produce a boolean from integer inputs; equality, comparison, all-different, etc.|
| Literal -> SATInt                   | 4500     | These are the rules that convert literals integer to SATInts, they should be applied last to allow for other rules to perform optimisations, for example multiplying by a known constant requires significantly less clauses and auxillary variables then treating the literal as an unkown integer. |
| Boolean -> Boolean                  | 4400     | These are the rules for boolean-to-boolean operations; and, or, not, xor, etc. |
| Boolean -> Nothing                  | 4300     | These are rules that take booleans and fully remove them from the constraints, this occurs when an expression if fully represented in the clauses. |

# SAT Rule Implementation Patterns

## Overview

Most SAT rules in conjure-oxide follow the same small set of implementation patterns. The details change from rule to rule, but the structure is usually the same: validate the input, normalise the operands, build a compact Boolean construction, convert it to CNF with Tseytin helpers, and propagate the output domain if the result is an integer.

This page collects the reusable techniques so the rule-specific pages can stay short. If you want the exact current implementation, treat the code as the source of truth and use the links below.

## Generic techniques

1. **Normalise operands first** - most integer rules pad bit-vectors to a shared range so later zips or index lookups are safe.
2. **Exploit the encoding shape** - direct encoding prefers bucketed OR/AND logic over arithmetic; order encoding prefers prefix comparisons; logarithmic encoding leans on bitwise circuits.
3. **Short-circuit degenerate cases** - empty sums, empty buckets, and single-element buckets can usually return a constant or a direct input bit without extra clauses.
4. **Use Tseytin helpers for Boolean structure** - once the high-level rule is identified, the actual CNF comes from `tseytin_and`, `tseytin_or`, `tseytin_not`, `tseytin_xor`, and friends.
5. **Propagate the domain** - integer outputs must update their range so later rules know the actual value interval.

## Current rule patterns

| Rule | Shared pattern | Current code |
| --- | --- | --- |
| Direct encoding summation | Common-range normalisation + pairwise DNF-style accumulation | `crates/conjure-cp-rules/src/sat/direct_int_ops.rs` (`add_sat_direct`) |
| Direct encoding absolute value | Bucket by \|value\|, then OR each bucket | `crates/conjure-cp-rules/src/sat/direct_int_ops.rs` (`abs_value_sat_direct`) |
| Order encoding inequality | Prefix comparison using the first position where `lhs` is false and `rhs` is true | `crates/conjure-cp-rules/src/sat/order_int_ops.rs` (`validate_order_int_operands`, `sat_order_lt`, `ineq_sat_order`) |

## Rule pages

The rule-specific pages are intentionally brief. They give the shape of the rule, point to the implementation, and leave the exact clause-by-clause details to the code.

If you are changing a rule, read the current implementation first and then update the page only if the high-level pattern changes.

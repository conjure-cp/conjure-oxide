# Direct Encoding SAT Summation

## Overview

```text
Sum(SATInt(a), SATInt(b), ...) ~> SATInt(c)
```

## What this rule does

- Normalises all operands to a shared value range.
- Builds the sum pairwise, using `tseytin_and` for each value pair and `tseytin_or` to accumulate matching terms.
- Propagates the resulting range after every addition step.
- Handles the empty-input case by returning the constant zero.

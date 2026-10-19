# Log Encoding allDiff

The pairwise formulation below is one selectable composition. The SAT IR also supports per-value AMO when every operand has an actual-value choice view. Both compositions retain full reification and support an exceptional value through allDifferentExcept.

## Overview

```text
allDiff(SATInt(a), SATInt(b), ...) ~> bool
```

## Rule Method
- Iterate over all possible pairs of operands
- Create a conjunction over the inequality of every pair

## Formal Notation

$$
\begin{align}
\text{allDiff} (A_1, A_2, \dots,  A_n) \quad \equiv \quad \bigwedge\limits_{1 \leq i < j \leq n}^{} (A_i \neq A_j)
\end{align}
$$

[//]: # (Author: Vlad Tronciu)
[//]: # (Last Updated: 22/05/2026)

# Constraint Dominance Programming (CDP)

## What CDP Means in Conjure Oxide
In Conjure Oxide, **Constraint Dominance Programming (CDP)** is used to enumerate solutions while filtering out those that are dominated by other solutions under a user-defined dominance relation.

At a high level:

1. Solve the model and obtain a solution.
2. Add a new constraint that blocks any future solution dominated by that solution.
3. Continue until no more solutions remain (or the user stops search).

This is how we obtain a set of non-dominated solutions (Pareto-optimal w.r.t. the chosen dominance relation).

## Language Additions

### `dominance relation`
The model can now include a top-level dominance section:

```text
dominance relation
    ...
```

The parser stores this in `Model.dominance`.

### `fromSolution(...)`
Inside a dominance relation, `fromSolution(x)` refers to the value of `x` in a previously found solution.

Example:

```text
dominance relation
    (x <= fromSolution(x)) /\
    (x < fromSolution(x))
```

`fromSolution(...)` is only valid inside `dominance relation`.

### `pareto(...)` syntax
To avoid writing dominance relations manually, the parser supports:

```text
dominance relation
    pareto(minimising x, maximising y)
```

Each item declares a component and direction:

- `minimising expr`
- `maximising expr`

## How `pareto(...)` Is Lowered
`pareto(...)` is parsed as syntactic sugar and lowered into a standard dominance expression in the AST.

For each component, the parser builds:

- a **non-worsening** condition, and
- a **strict improvement** condition.

Then it combines all components as:

```text
AND(all non-worsening) AND OR(any strict improvement)
```

For integer components:

- `minimising e` becomes `e <= fromSolution(e)` and strict `e < fromSolution(e)`
- `maximising e` becomes `e >= fromSolution(e)` and strict `e > fromSolution(e)`

For boolean components:

- `minimising b` becomes `b -> fromSolution(b)` and strict `(!b) /\ fromSolution(b)`
- `maximising b` becomes `fromSolution(b) -> b` and strict `b /\ !fromSolution(b)`

Notes:

- `pareto(...)` is only allowed inside `dominance relation`.
- Components must have return type `int` or `bool`.
- Explicit `fromSolution(...)` inside `pareto(...)` components is rejected.

## Why Solver Adaptors Needed Changes
Supporting CDP required adaptors to evolve from "load once, solve once" behaviour into iterative solving with incremental constraint addition.

After each solution, adaptors construct:

```text
NOT dominance(current_solution, future_solution)
```

where references in the dominance expression are rewritten as follows:

- current-solution references are substituted with concrete literals,
- `fromSolution(x)` is rewritten to refer to the future candidate variable `x`,
- the whole expression is negated to block dominated futures.

This rewritten expression is then fed back to the backend during the same solve run.

## Backend-Specific CDP Implementation

### SAT (`rustsat` / CaDiCaL)
- Rewrites the dominance block into CNF through the normal rewrite pipeline.
- Adds resulting clauses incrementally to the SAT solver.
- Extends SAT variable mapping when dominance clauses introduce new references.

### SMT (Z3)
- Rewrites the dominance block through the same model-rewrite pipeline.
- Asserts rewritten constraints into the active Z3 solver instance between solutions.

### Minion
- Rewrites dominance to a temporary model and lowers it to Minion constraints.
- Adds auxiliary Minion variables mid-search as needed.
- Injects remapped Minion constraints mid-search.

This path required dedicated mid-search injection support and robust handling around Minion runtime constraints.

## CLI and Tracing Notes
- `--rule-trace-cdp` controls whether solver-time CDP rewrites are included in rule traces.
- The CLI solution pipeline still applies a final dominance-pruning pass over collected solutions as a safety net.

## Related Minion Follow-Ups
In addition to core CDP support, related Minion work included:

- `--minion-valorder` to control Minion value ordering (`ascend`, `descend`, `random`) from the CLI.
- adaptor robustness fixes in callback/solution handling.
- dedicated regression tests around mid-search variable/constraint injection behaviour.

## Testing Coverage
CDP behaviour is exercised in integration tests under:

- `tests-integration/tests/integration/dominance/`

These include both explicit dominance formulas and `pareto(...)` syntax, across integer and boolean cases, and across multiple solver families.

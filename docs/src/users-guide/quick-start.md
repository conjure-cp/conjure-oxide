# Quick Start Guide to Running your first Essence Model

This guide walks you through running your first Essence model with Conjure Oxide.

## Your First Problem

Create a file called `my_problem.essence` with the following content:

```essence
find x : int(1..3)
find y : int(2..5)

such that x > y
```

If you are curious about more complex models, you can check out the models that we use to test Conjure Oxide, available in the `test-suite/tests/integration` directory of the repository.

## Running with Different Solvers

`--solver` takes `minion`, `sat`, or `z3`.

How a model is *expressed* for the chosen solver is separate from which solver it is. SAT has no
integers, so each integer variable is encoded into Booleans as `int_log`, `int_direct` or
`int_order`; Z3 has integers but two theories to hold them in, `lia` or `bv`. Both are
representation choices made per declaration, exactly like choosing `occurrence` or `explicit` for a
set, and both are steered with `--heuristic` (see the modelling-choices guide). Variables in
different encodings are channelled together automatically where a constraint needs them to agree.

### SAT Solver

```bash
cargo run -- solve --solver sat my_problem.essence
```

### Z3 (SMT)

```bash
cargo run -- solve --solver z3 my_problem.essence
```

### Minion Solver

```bash
cargo run -- solve --solver minion my_problem.essence
```

**Expected output for both solvers:**

```json
Solutions:
[
  {
    "x": {
      "Int": 3
    },
    "y": {
      "Int": 2
    }
  }
]
```

## Understanding What Happened

Conjure Oxide transformed your high-level Essence model through several steps:

1. **Parsing** - Your Essence file was parsed into an internal AST
2. **Rule Application** - Backend-specific rules transformed the model
3. **Solving** - The transformed model was sent to the solver
4. **Solution Extraction** - The solver's output was converted back to Essence format

Want to see exactly what rules were applied? Check out the [Logging guide](command-line-guide/logging.md).

## Functional Programming Style

For developers who come from programming languages like Scala or Haskell, or those who favour a functional programming style, we have a [Functional Rust](../developers-guide/resources-conventions/functional-rust.md) guide that you might find useful.

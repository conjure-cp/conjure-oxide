# Quick Start Guide

This guide walks you through running your first Essence model with Conjure Oxide. 

## Your First Problem

Create a file called `my_problem.essence` with the following content:

```essence
find x :  int(1..3)
find y : int(2..5)

such that x > y
```

## Running with Different Solvers

### SAT Solver

```bash
cargo run -- solve --solver sat my_problem.essence
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

Want to see exactly what rules were applied? Check out the [Logging guide](./command-line/logging.md).
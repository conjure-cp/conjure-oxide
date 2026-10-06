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
integers, so each integer variable is encoded into Booleans as `direct`, `order`, or one of four
binary forms -- `twos_complement`, `sign_magnitude`, `offset` (the distance from the domain minimum)
or `rank` (the position among the allowed values); Z3 has integers but two theories to hold them in, `lia` or `bv`. Both are
representation choices made per declaration, exactly like choosing `occurrence` or `explicit` for a
set, and both are steered with `--heuristic` (see the modelling-choices guide). Variables in
different encodings are channelled together automatically where a constraint needs them to agree.

### Choosing a representation in the model

Any domain can carry a representation preference, written as an attribute straight after the
domain's first word, so the choice sits next to the declaration it applies to. A declaration with a
preference is given that representation instead of asking the heuristic.

```essence
find a : int(representation order, 1..4)
find s : set (representation occurrence) of int(1..9)
find m : matrix (representation packed) indexed by [int(1..3)] of bool
find t : tuple (representation components, int(1..3), bool)
find r : record (representation packed) {x: int(1..3), y: bool}
find f : function (representation explicit, total) int(1..3) --> int(1..3)
```

`int` and `tuple` put the preference first inside their parentheses, because those already hold the
ranges or the member domains; `int(representation order)` leaves the ranges unbounded. Every other
domain keeps it in the attribute list that follows the keyword (`matrix`, `record` and `variant`
take it as their only attribute). The interactive heuristic (`-hi`) prints each option in this
syntax, so a choice you make at the prompt can be pasted into the model to make it permanent.

### Uniform representation choices

Use `--channelling uniform` to choose one representation kind for each type family throughout
one model. All integers use the same encoding regardless of their bounds; all sets use the same
layout regardless of their element domains. Nested representation variables follow the same
policy. Matrix layout and integer encoding are separate choices.

```bash
cargo run -- solve --solver sat --channelling uniform --heuristic i my_problem.essence
```

The interactive heuristic prompts once for each undecided type family. An incompatible domain is reported
rather than silently switching representation. `--channelling no` keeps the existing policy of
one representation per variable, and `yes` permits multiple channelled representations.

Integration-test configurations can use `channelling = "uniform"` with `heuristic = "x"` to
exercise the available combinations without choosing independently for each integer variable.

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

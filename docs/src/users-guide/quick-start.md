# Quick Start Guide to Running your first Essence Model

This guide walks you through running your first Essence model with Conjure Oxide.

## Your First Problem

Once you have gone through the [installation](installation.md) process and got Conjure Oxide on your computer you can solve your first problem.

Suppose you want to find two numbers `x` and `y` so that they add up to 15 and the difference between them is 3. The mathematicians will immediately reach for their simultaneous equations and start doing arithmetic.

\\[ x + y = 15 \\]
\\[ x - y = 3 \\]

We, however, take the lazier approach.

Create a file called `my_problem.essence` with the following content:

```essence
find x : int(1..20)
find y : int(2..20)

such that x + y = 15,
          x - y = 3
```

In your terminal run this command:

```bash
$ conjure-oxide solve my_problem.essence
```


**Expected output:**

```
$ Solution: 000001
language Essence 1.3

letting x be 9
letting y be 6
```

We see that this indeed satisfies our constraints with \\[ 9 + 6 = 15 \\] and \\[9 - 6 = 3\\]

## Understanding the structure of the problem specification

The first two lines are where we declare our *decision variables*. These are the things which we want to find.

These are declared as:
```
find <name> : <domain>
```
Where the domain tells Conjure Oxide what type and specifically what values the variable could possibly take on.

The last statement contains our constraints as a comma-separated list of boolean expressions.

The goal of Conjure Oxide is to give you values for all of your decision variables, within the allowed domains, that also make all of the boolean expressions evaluate to true.

## Understanding What Conjure Oxide did

Conjure Oxide transformed your high-level Essence model through several steps:

1. **Parsing** - Your Essence file was parsed into an internal AST
2. **Rule Application** - Backend-specific rules transformed the model
3. **Solving** - The transformed model was sent to the solver
4. **Solution Extraction** - The solver's output was converted back to Essence format

Want to see exactly what rules were applied? Check out the [Logging guide](command-line-guide/logging.md).

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

Use `--solver sat` and select a compiled backend with `--sat-backend cadical` or `--sat-backend batsat`.

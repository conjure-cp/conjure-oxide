# Quick Start Guide to Running your first Essence Model

This guide walks you through running your first Essence model with Conjure Oxide.

## Your First Problem

Once you have gone through the [installation](installation.md) process and got Conjure Oxide on your computer you can solve your first problem.

Suppose you want to find two numbers `x` and `y` so that they add up to 15 and the difference between them is 3. The mathematicians will immediately reach for their simultaneous equations and start doing arithmetic

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

In your terminal run this command

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

These are declared as
```
find <name> : <domain>
```
where the domain tells Conjure Oxide what type and specifically what values the variable could possibly take on.

The last statement contains our constraints as a comma-separated list of boolean expressions

The goal of Conjure Oxide is to give you values for all of your decision variables, within the allowed domains, that also make all of the boolean expressions evaluate to true.

## Understanding What Conjure Oxide did

Conjure Oxide transformed your high-level Essence model through several steps:

1. **Parsing** - Your Essence file was parsed into an internal AST
2. **Rule Application** - Backend-specific rules transformed the model
3. **Solving** - The transformed model was sent to the solver
4. **Solution Extraction** - The solver's output was converted back to Essence format

Want to see exactly what rules were applied? Check out the [Logging guide](command-line-guide/logging.md).

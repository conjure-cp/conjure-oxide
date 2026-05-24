# Problem and Parameter Files

**This document is a work in progress.**

To allow problems to be reused with different sets of parameters, Conjure Oxide allows you to provide a separate parameter file, in addition to the standard problem file.

The **problem file** defines the problem in its abstract form (e.g. "find z such that x < z < y"). Parameters are declared using `given` statements:

```essence
given <variable>: <domain>
```

The **parameter file** assigns concrete values to problem variables (e.g. x = 4, y = 7). Values are assigned using `letting` statements:

```essence
letting <variable> be <literal value>
```

Conjure oxide takes parameter files as a second (optional) positional argument:

```bash
conjure-oxide solve <problem file> <param file>
```

## Example

Given the following problem:

```essence
$ my_problem.essence
given x: int
given y: int
find z: int(1..10)
  such that (z > x) /\ (z < y)
```

...you can bind values to `x` and `y` using the following parameter file:

```essence
$ my_params.param
letting x be 4
letting y be 7
```

...and then solve the problem:

```bash
conjure-oxide solve \
  --solver via-conjure \
  --number-of-solutions all \
  my_problem.essence \
  my_params.param

# should output 5 and 6 as solutions
```

## Errors

If a problem file contains parameters that are not assigned in the parameter file, or the assigned value is not of the correct domain, Conjure Oxide will not be able to solve, and will report an error.

## `.param` vs `.essence` file types solve

Currently, in order to parse files containing `given` statements, Conjure Oxide must invoke the legacy Conjure program, which makes a distinction between `.param` files and `.essence` files for performance reasons.

Parameter files with the `.param` extension only have access to a subset the Essence language. For instance, letting statements may only assign literal values, rather than expressions. _For most use cases this should be fine, but if your parameter file needs to be a bit more complex, you may consider upgrading it to a `.essence` file._

Parameter files with the `.essence` extension may utilise the full essence language, including arithmetic/boolean expressions, comprehensions, and other expressive features. These may take longer to parse, so only consider downgrading simple parameter files to `.param`, especially when running a problem against many of them.

_In future, the native Essence parser will support `given` statements, so this distinction may be removed._

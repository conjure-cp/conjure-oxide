# Crate Structure

## Overview

We follow a "monorepo" approach; That is, a number of related modules are developed in a single repository.
This makes it easier to integrate them and test our entire project all at once.

Our repository can be broken down into four key components:

1. The user-facing `conjure-oxide` command line tool and library, and its integration tests.
   This is stored in `conjure-oxide/conjure_oxide`.
2. The AST type definitions, rewrite engine, and related implementation code.
   These are broken up into separate crates and stored in `conjure-oxide/crates/`.
3. Rust bindings for solvers; That is, wrapper code and build files that enable us to:
   - Compile solvers (which are developed outside of this project and written in other languages, such as C++) together with our project
   - "Hook into" solver methods and call them from inside our Rust code

     These are stored in `conjure-oxide/solvers/` on a per-solver basis.
4. Various other scripts and tools that we use to build, document and test our project.
   These are stored in `conjure-oxide/tools/`.

## CLI Tool

The command-line `conjure-oxide` tool is the final product we ship to users.
All logic related to the CLI, parsing user input, and displaying solutions is implemented in `conjure-oxide/conjure_oxide/src`.

This module (called a "crate" in Rust) also re-exports some type definitions and functions from `conjure_core` and other crates. 
This is our public, user-facing API that could eventually be used by other people in their projects. At the moment, our project is under development and there is no stable API specification.

There are some `examples/` illustrating how various parts of the project are used.
Finally, for historical reasons, this folder contains some utility code that may be refactored or moved elsewhere in the future.

## Tests

Our suite of integration tests runs automatically on every commit to the main repository.
It tests `conjure-oxide` end-to-end: parsing an example Essence file, running the rewriter, calling the solver, and validating the solution.

The test files are stored in `conjure_oxide/tests/integration/`, sorted into sub-directories.
Code to run our project on these files is contained in `integration_tests.rs`.

## Crates

Most of our implementation is contained in the `crates/` directory. Here is an overview of what each crate does:

- `conjure_core` contains:
  - Our rule engine implementation (`conjure_core/src/rule_engine/`)
  - The definition of our Abstract Syntax Tree (AST) for Essence; That is, the Rust representation of an Essence program (`conjure_core/src/ast`)
  - The generic `SolverAdaptor` interface for interacting with solvers (`conjure_core/src/solver`)
  - Concrete implementations of `SolverAdaptor` for each solver we support, such as Minion or RustSAT (`conjure_core/src/solver/adaptors`)
  - Other miscellaneous types and utilities
- `conjure_rules` defines rules and rule sets for our rewrite engine to use.
  Rules are grouped into files and directories based on their purpose: for example, all rules for normalising boolean expressions are in `conjure_rules/src/normalisers/bool.rs`
- `conjure_rule_macros` implements the `#[register_rule(...)]` and `#[register_rule_set(...)]` procedural macros. See also: [Wiki - Rules and RuleSets](https://github.com/conjure-cp/conjure-oxide/wiki/Expression-rewriting%2C-Rules-and-RuleSets).
- `conjure_essence_parser` implements the native Rust parser for Essence.
  It uses our Tree-sitter grammar, which is defined *separately* in`tree-sitter-essence`
- `conjure_essence_macros` implements the `essence_expr!` procedural macro.
- `enum_compatability_macro` is a macro that allows us to indicate whether certain features of Essence are compatible with certain solvers, for documentation purposes.
- `randicheck` is a somewhat separate project developed by Ty (@TAswan) and others.
  It aims to use Conjure to automatically validate Haskell code and generate minimal failing tests. (TODO is this accurate?)
- `tree_morph` is a generic library for tree transformations.
  In the future, it will replace our current rule engine implementation.

Also:

- The `uniplate` crate, which we use to traverse the AST, used to be part of this repository.
  It is now maintained separately at https://github.com/conjure-cp/uniplate.



## Dependencies

The dependencies between crates are shown bellow.

![graphviz(3)](https://github.com/user-attachments/assets/ffda823b-b5f9-4fa8-b66e-e74d2c08a75b)

An arrow `A -> B` means that `A` imports from `B`. The diagram is made using Graphviz, and its source code is located TODO.
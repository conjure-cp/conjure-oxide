# Conjure Oxide

This repository contains the in-progress Conjure Oxide constraints modelling
tool and its dependencies. 

<https://conjure-cp.github.io/conjure-oxide/>

## Installation

### Building from Source

The following dependencies are required:

* [Conjure](https://github.com/conjure-cp/conjure) (including solvers)
* Clang 
* Libclang
* Cmake
* Rust, installed using [rustup](https://rustup.rs/)

**Ensure that Conjure is placed early in your PATH to avoid conflicts with
ImageMagick's conjure command!**

Run `cargo install --path crates/conjure-cp-cli` to install `conjure-oxide`.

## Repository Structure

This repository holds the source-code for both `conjure-oxide` itself, and
various related projects.

### `conjure-oxide` crates

The following crates define the `conjure-oxide` system:

- [`conjure-cp`](./crates/conjure-cp) defines `conjure-oxide` as a library. It
  re-exports symbols from the following internal crates:

  + [`conjure-cp-core`](./crates/conjure-cp-core)
  + [`conjure-cp-enum-compatibility-macro`](./crates/conjure-cp-enum-compatibility-macro)
  + [`conjure-cp-essence-macros`](./crates/conjure-cp-essence-macros)
  + [`conjure-cp-essence-parser`](./crates/conjure-cp-essence-parser)
  + [`conjure-cp-rule-macros`](./crates/conjure-cp-rule-macros)


- [`conjure-cp-cli`](./crates/conjure-cp-cli) implements the `conjure-oxide`
  command line interface, and exports various CLI related utilities for use by
  the integration tester.
- [`conjure-cp-rules`](./crates/conjure-cp-rules) defines the default rewrite
  rules used by `conjure-oxide`.
- [`tests-integration`](./tests-integration) is an internal crate containing
  integration tests for `conjure-oxide`.

### Ecosystem crates

The following crates are related to, or used by, `conjure-oxide`, but can be
used in isolation from it:

- [`minion-sys`](./crates/minion-sys) defines FFI bindings for the [Minion CP solver](https://github.com/minion/minion).
- [`tree-morph`](./crates/tree-morph) provides a framework for implementing
  term-rewriting systems. 
- [`tree-sitter-essence`](./crates/tree-sitter-essence) defines a tree-sitter
  grammar for Essence.
- [`randicheck`](./crates/randicheck)

### Related Projects 

The following projects are used by, and developed alongside, `conjure-oxide`,
but are kept in their own repositories:

- [`uniplate`](https://github.com/conjure-cp/uniplate)


## Licence

This project is being produced by staff and students of University of St
Andrews, and is licenced under the [Mozilla Public Licence 2.0](./LICENCE).

<!-- vim: cc=80
-->


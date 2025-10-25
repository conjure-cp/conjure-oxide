# `minion-sys`

TODO: Not sure where are these meant to come from.
<!-- md-dead-link-check: off -->
[![Coverage Badge](https://conjure-cp.github.io/conjure-oxide/coverage/main/minion/badges/flat.svg)](https://conjure-cp.github.io/conjure-oxide/coverage/main/minion/)
[![solvers/minion](https://github.com/conjure-cp/conjure-oxide/actions/workflows/minion.yml/badge.svg?event=push)](https://github.com/conjure-cp/conjure-oxide/actions/workflows/minion.yml)
<!-- md-dead-link-check: on -->

This crate contains (in progress) Rust bindings for the [Minion](https://github.com/minion/minion) constraint solver.

Read the documentation [here](https://conjure-cp.github.io/conjure-oxide/docs/minion_sys/index.html).

## Licence

This crate is licensed under the [Mozilla Public Licence 2.0](https://www.mozilla.org/en-US/MPL/2.0/).

## Debugging

Debug symbols for Minion can be enabled by setting the environment variable `DEBUG_MINION`.

Eg.

```shell
DEBUG_MINION=true cargo test
```

# Domains

## What are Domains and Types

A **Domain** is the discrete and finite set of values that a variable or expression can take.

For information on domains, please see the [Essence Reference](/reference/domains/domains.md).

A **Type** is a more general description about some collection.

## Ground and Unresolved

Looking at `conjure-cp-core::ast::domains`, there is a `Domain` Enum, with variants `Ground` and `Unresolved`.

- An `Unresolved` domain is a domain whose bounds are tied to an expression that has not been evaluated. For example `x: int(1..(2+1))` and `x: int(1..n)` are both unresolved.
- A `Ground` domain is a domain entirely composed of literals. For example: `int(2..5)`, `set (maxSize 2) of int(1..3)`.

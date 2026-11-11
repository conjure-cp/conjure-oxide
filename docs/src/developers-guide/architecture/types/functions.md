[//]: # "Author: Nicholas Davidson"
[//]: # "Last Updated: 10/02/2026"

# Functions

## What Are Functions

Please see the [Essence Reference](/reference/domains/functions.md).

Functions have attributes and two domains for the domain and codomain of its results. They are defined for both `GroundDomain` and `UnresolvedDomain` as `Function(FuncAttr, Moo<GroundDomain>, Moo<GroundDomain>)`.

The result of a function is a list of tuples defined by the mappings. As such the `AbstractLiteral` for functions is defined `Function(Vec<(T, T)>)`. This AbstractLiteral type means that functions can also be represented explicitly.

## Note on Implementation

Currently functions are defined within the AST of Conjure-Oxide and can be parsed with the 'legacy' parser. However, there is no support as of Apr-2026 for rewriting rules or solving.

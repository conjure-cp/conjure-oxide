[//]: # (Author: Calum Cooke)
[//]: # (Last Updated: 18/04/2026)

# Sequences
## What Are Sequences
A sequence is a datatype for storing a series of values, where values can occur multiple times and ordering is preserved. It differs from a [matrix](https://github.com/conjure-cp/conjure-oxide/wiki/), in that:
- Not recursive; i.e. the inner domain of a sequence cannot be another high-level type, it must be either an integer or a boolean.
- Can have defined attributes; i.e. you can define a sequence to have varying lengths, and to restrict what values it can draw from it's inner domain.


## Attributes
Three **cardinality** attributes; `size`, `minSize`, `maxSize`.
Three **jectivity** attributes; `injective`, `surjective`, `bijective`.

Sequences _must_ have either a `size` or `maxSize` cardinality attribute. 

## Operators

There are two operators which are defined on functions. These are represented as `Expressions` in Conjure-Oxide.

- `subsequence`: does the sequence `s` appear in the same order in `t` (e.g. `s=1,2,3` and `t=1,3` are subsequences)
- `substring`: does the sequence `s` appear in the same order _and_ contiguously in `t` (e.g. `s=1,2` is a substring of `t=1,2,3` )

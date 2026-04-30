[//]: # (Author: Calum Cooke)
[//]: # (Last Updated: 18/04/2026)

# Sequences
## What Are Sequences
A sequence is a datatype for storing a series of values, where values can occur multiple times and ordering is preserved. It differs from a [matrix](https://github.com/conjure-cp/conjure-oxide/wiki/), in that it can have defined attributes; i.e. you can define a sequence to have varying lengths, and to restrict what values it can draw from it's inner domain.


## Attributes
* A **cardinality** attribute, as a range; `size`, `minSize`, `maxSize`.
* A **jectivity** attribute, with three options; `injective`, `surjective`, `bijective`.

Sequences _must_ have either a `size` or `maxSize` cardinality attribute. 

The cardinality attribute is used with some value (i.e. `size 4`), whereas the jectivity attribute is used by itself.
For example;
```
$ Valid
find foo: sequence of int(1..5)
find bar: sequence (size 5, surjective) of int(1..5)
find fizz: sequence (minSize 2, maxSize 7, injective) of int(1..10)
find buzz: sequence (bijective) of int(1..5)

$ Syntactically Invalid
find biff: sequence (minSize 3, size 4) of int(2..7) $ Cardinality attribute cannot be single and not single
```

## Operators

There are two operators which are defined on sequences. These are represented as `Expressions` in Conjure-Oxide.

- `subsequence`: does the sequence `s` appear in the same order in `t` (e.g. `s=1,2,3` and `t=1,3` are subsequences)
- `substring`: does the sequence `s` appear in the same order _and_ contiguously in `t` (e.g. `s=1,2` is a substring of `t=1,2,3` )

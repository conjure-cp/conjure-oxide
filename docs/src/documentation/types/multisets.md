[//]: # (Author: Calum Cooke)
[//]: # (Last Updated: 09/12/2025)

# Multisets
## What Are Multisets
A multiset is a datatype for storing a set of objects where objects can occur more than once, but the ordering of objects does not matter.
Multisets have attributes and a single domain. A multiset can be defined for both `GroundDomain` and `UnresolvedDomain`.

## Attributes
Three **cardinality** attributes; `size`, `minSize`, `maxSize`.
Two **occurrence** attributes; `minOccur`, `maxOccur`.

In the original conjure implementation, a multiset was _infinite_ without `size`, `maxSize`, or `maxOccur`. 
In the new conjure-oxide implementation, a variable's domain is _ground_ if it is fully-bounded (i.e. has a `minSize` and `maxSize`).

## Operators

There are four operators which are defined on functions. These are represented as `Expressions` in Conjure-Oxide.

- `hist(m)` - histogram of multi-set `m`
- `max(m)` - largest element in ordered multiset `m`
- `min(m)` - largest element in ordered multiset `m`
- `freq(m,e)` - counts occurrences of element `e` in multiset `m`


## Note on Implementation
Multisets are **under-development** for the legacy parser. There is also no support as of Dec-2025 for rewriting rules or solving.
# Multisets
## What Are Multisets
A multiset is a datatype for storing a set of objects where objects can occur more than once, but the ordering of objects does not matter.
Multisets have attributes and a single domain. A multiset can be defined for both `GroundDomain` and `UnresolvedDomain`.

## Attributes
* A **cardinality** attribute, as a range; `size`, `minSize`, `maxSize`.
* An **occurrence** attribute, as a range; `minOccur`, `maxOccur`.

In the original conjure implementation, a multiset was _infinite_ without `size`, `maxSize`, or `maxOccur`. 
In the new conjure-oxide implementation, a variable's domain is _ground_ if it is fully-bounded (i.e. has a `minSize` and `maxSize`).

```
$ Valid
find foo: multiset (maxSize 5, minOccur 2) of int(1..10)
find bar: multiset (size 3) of int(1..4)


$ Invalid
find fizz: multiset of int(1..10)               $ there must be a upper-bounded size attribute
find buzz: multiset (minSize 3) of int(1..10)   $ this is a lower bound, there is no upper bound on the size attribute
```

## Operators

There are four operators which are defined on functions. These are represented as `Expressions` in Conjure-Oxide.

- `hist(m)` - histogram of multi-set `m`
- `max(m)` - largest element in ordered multiset `m`
- `min(m)` - largest element in ordered multiset `m`
- `freq(m,e)` - counts occurrences of element `e` in multiset `m`

> As of 29th April 2026, `Min` and `Max` have been added as Expressions, but `Hist` and `Freq` have not.
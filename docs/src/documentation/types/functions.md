[//]: # (Author: Nicholas Davidson)
[//]: # (Last Updated: 09/12/2025)

# Functions
## What Are Functions
A function is a binary relation on two sets, from elements of a domain to elements of a codomain. 

Functions have attributes and two domains for the domain and codomain of its results. They are defined for both `GroundDomain` and `UnresolvedDomain` as `Function(FuncAttr, Moo<GroundDomain>, Moo<GroundDomain>)`.

The result of a function is a list of tuples defined by the mappings. As such the `AbstractLiteral` for functions is defined `Function(Vec<(T, T)>)`. This AbstractLiteral type also means that functions can also be represented explicitly.

## Attributes

Functions contain three types of attributes: size, partiality and jectivity.

Size attributes determine how many mappings the function is true for. Size attributes are converted to a `Range<A>` type in Conjure-Oxide. This size can be defined as:
- A single size `size(x)`
- A minimum size `minSize(x)`
- A maximum size `maxSize(x)`
- A range of sizes `minSize(x) maxSize(x)`
- Or unbounded by not specifying any attribute

Partiality determines whether the function is total or partial. A total function is one which is defined for every possible input in the domain set. A function will have partial partiality unless specified otherwise.
Partiality is stored as an Enum inside Conjure-Oxide.

Jectivity refers to whether a function is injective, surjective, or bijective. These determine whether elements in the domain and codomain must have only one corresponding mapping. A function can also have no specified jectivity, which is the default.
Jectivity is also stored as an Enum inside Conjure-Oxide.

## Operators

There are seven operators which are defined on functions. These are represented as `Expressions` in Conjure-Oxide.

- Defined(f) - Returns the set of values in the domain for which a function f is defined.
- Image(f, x) - Returns the element of the codomain which is mapped to domain element x, in function f.
- ImageSet(f, x) - Returns the set of elements of the codomain which is mapped to domain element x, in function f.
- Inverse (f1, f2) - Returns a boolean representing if functions f1 and f2 are inverses of each other.
- PreImage(f, x) - Returns the set of elements of the domain which map to codomain element x, in function f.
- Range(f) - Returns the set of values in the domain for which a function f is defined.
- Restrict(f, D) - Returns a sub-function of f which has its mapping restricted to values in the domain D.

## Note on Implementation

Currently functions are defined within the AST of Conjure-Oxide and can be parsed with the 'legacy' parser. However, there is no support as of Dec-2025 for rewriting rules or solving.
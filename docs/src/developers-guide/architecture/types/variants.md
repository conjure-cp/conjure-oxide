[//]: # "Author: Nicholas Davidson"
[//]: # "Last Updated: 23/04/2026"

# Variants

## What Are Variants

Please see the [Essence Reference](/reference/domains/variants.md).

Variants are defined for both `GroundDomain` and `UnresolvedDomain`, and stored as `Variant(Vec<FieldEntry>)`.

The result of a variant is a single field value. As such the `AbstractLiteral` for variants is defined as `Variant(Moo<FieldValue<T>>)`. This AbstractLiteral type means variants can be represented explicitly.

## Operators

TODO move to reference

Variants can also be indexed, which makes use of Conjure-Oxide's `UnsafeIndex` and `SafeIndex`.

## Note on Implementation

Currently variants are defined within the AST of Conjure-Oxide and can be parsed with the 'legacy' parser. However, there is no support as of Apr-2026 for rewriting rules or solving.

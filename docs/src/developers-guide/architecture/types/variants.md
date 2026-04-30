[//]: # (Author: Nicholas Davidson)
[//]: # (Last Updated: 23/04/2026)

# Variants
## What Are Variants
A variant domain stores key value domain pair fields. However, unlike a record, a variant literal can only use one of these fields at once. This field is referred to as active.

Variants contain a list of fields, with domains, which they can take. They are defined for both `GroundDomain` and `UnresolvedDomain`, and stored as `Variant(Vec<FieldEntry>)`.

The result of a variant is a single field value. As such the `AbstractLiteral` for variants is defined as `Variant(Moo<FieldValue<T>>)`. This AbstractLiteral type means variants can be represented explicitly.

## Operators

There is one special operator which is defined on only variants.

- `Active(v, f)` - Returns whether field f is currently the field active in variant v.

Variants can also be indexed, which makes use of Conjure-Oxide's `UnsafeIndex` and `SafeIndex`.

## Note on Implementation

Currently variants are defined within the AST of Conjure-Oxide and can be parsed with the 'legacy' parser. However, there is no support as of Apr-2026 for rewriting rules or solving.
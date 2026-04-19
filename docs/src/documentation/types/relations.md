[//]: # (Author: Nicholas Davidson)
[//]: # (Last Updated: 19/04/2026)

# Relations
## What Are Relations
A relation is effectively a set of tuples. The internal tuples can be of any arity and can have different internal domains, as long as every tuple in the relation has the same airty and domains. If a relation is binary we can also add additional attributes to define properties of the relation.

Relations have attributes and a list of domains for inside the tuples. They are defined for both `GroundDomain` and `UnresolvedDomain` as `Relation(RelAttr, Vec<GroundDomain>)`.

The result of a relation is a list of tuples. As such the `AbstractLiteral` for relations is defined `Relation(Vec<Vec<T>>)`. This AbstractLiteral type means that relations can also be represented explicitly.

## Attributes

Relations contain two types of attributes: size and binary attributes.

Size attributes determine how many tuples are in the relation. Size attributes are converted to a `Range<A>` type in Conjure-Oxide. This size can be defined as:
- A single size `size x`
- A minimum size `minSize x`
- A maximum size `maxSize x`
- A range of sizes `minSize x maxSize x`
- Or unbounded by not specifying any attribute

Binary attributes determine properties of the relation when it is a binary relation. Binary attributes are stored as a list of attributes `Vec<BinaryAttr>` and any number of them can be provided. The avaliable binary attributes are as follows:
- `reflexive`
- `irreflexive`
- `coreflexive`
- `symmetric`
- `antiSymmetric`
- `aSymmetric`
- `transitive`
- `total`
- `connex`
- `Euclidean`
- `serial`
- `equivalence`
- `partialOrder`

## Operators

There is one special operator which is defined on relations.

- `RelationProj(r, ps)` - Returns a new relation after projecting projectors ps onto relation r.<br>
The projectors ps are stored a `Vec<Option<Expression>>`, and the projecting components (denoted `_` in Essence) are stored as `None`<br>
For example: `r(_,1,_)` converts r from a 3-component relation to a 2-component relation, by projecting the 1st and the 3rd columns. It chooses entries where the second column is 1. This is equivalent to the SQL select query: `SELECT one, three FROM r WHERE two = 1`

## Note on Implementation

Currently relations are defined within the AST of Conjure-Oxide and can be parsed with the 'legacy' parser. However, there is no support as of Apr-2026 for rewriting rules or solving.
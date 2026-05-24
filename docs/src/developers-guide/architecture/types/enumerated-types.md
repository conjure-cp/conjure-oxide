# Enumerated Types

## What are Enumerated Types

An enumerated type is a type with several distinct, ordered members.

Any information which consists of known named members may be represented as an enumerated type, e.g. days of the week, cardinal directions, colours.

An enumerated type is declared in Essence as follows:

```essence
letting Direction be new type enum {North, East, South, West}
```

## Operators

There are two operations on enumerated types:

- `pred(m)` returns the predecessor of enum member `m`.
- `succ(m)` returns the successor of enum member `m`.

## Representation

Representing enumerated types in Conjure Oxide requires three different kinds of declaration.

- The type itself e.g. `Direction`. This is represented as its own declaration in the symbol table of kind `EnumeratedType`, which holds the type and variants' identifiers. We represent the above `Direction` enum as follows:

  ```rs
  DeclarationKind::EnumeratedType(
    EnumeratedType {
      name: "Direction",
      variants: vec!["North", "East", "South", "West"]
    }
  )
  ```

- Members of this type, e.g. `North`, `South`, `East` and `West`. Enum members are unqualified: referenced directly by name, without specifying the enum they belong to. Therefore, each member is its own value letting, referencing the enumerated type and the member's index. North, for example, is represented as follows:

  ```rs
  DeclarationKind::ValueLetting(
    // ...
    Literal::EnumVariant { ty: Moo(&Direction), variant: 0 }
    // ...
  )
  ```

- Domains over the type, e.g. `Direction(North..East)`. These are simply a domain letting, referencing the type and a set of ranges over its variants, e.g.:

  ```rs
  DeclarationKind::DomainLetting(
    // ...
    UnresolvedDomain::EnumeratedType(
      &Direction,
      vec![Range::Bounded(&North, &East)]
    )
    // ...
  )
  ```

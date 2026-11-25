# Domains

## What are Domains and Types
A **Domain** is the discrete and finite set of values that a variable or expression can take. A **Type** is a more general description about some collection. The simplest domains are **Concrete** domains: `empty`, `boolean`, `int(min..max)`. **Abstract** domains use some high-level types (such as a `Set`, `Matrix`, etc) to instantiate a domain where the objects of the domain contain more objects. These objects are called **Literals**; a literal is _one_ specific value taken from a domain.


In the original language description, found in the [Conjure Docs](https://conjure.readthedocs.io/en/latest/essence.html)
```
Domain := "bool"
        | "int" list(Range, ",", "()")
        | "int" "(" Expression ")"
        | Name list(Range, ",", "()") # the Name refers to an enumerated type
        | Name                        # the Name refers to an unnamed type
        | "tuple" list(Domain, ",", "()")
        | "record" list(NameDomain, ",", "{}")
        | "variant" list(NameDomain, ",", "{}")
        | "matrix indexed by" list(Domain, ",", "[]") "of" Domain
        | "set" list(Attribute, ",", "()") "of" Domain
        | "mset" list(Attribute, ",", "()") "of" Domain
        | "function" list(Attribute, ",", "()") Domain "-->" Domain
        | "sequence" list(Attribute, ",", "()") "of" Domain
        | "relation" list(Attribute, ",", "()") "of" list(Domain, "*", "()")
        | "partition" list(Attribute, ",", "()") "from" Domain

Range := Expression
       | Expression ".."
       | ".." Expression
       | Expression ".." Expression

Attribute := Name
           | Name Expression

NameDomain := Name ":" Domain
```


## Ground and Unresolved
Looking at `conjure-cp-core::ast::domains`, there is a `Domain` Enum, with variants `Ground` and `Unresolved`. 

* An `Unresolved` domain is a domain whose bounds are tied to an expression that has not been evaluated. For example `x: int(1..(2+1))` and `x: int(1..n)` are both unresolved.
* A `Ground` domain is a domain entirely composed of literals. For example: `int(2..5)`, `set (maxSize 2) of int(1..3)`. 



## Attributes
Abstract domains tend to also have attributes defined on them, which often restricts the possible values of the domain. 

> When defining a Set type, it may have attributes and an inner domain. For example, the domain `set (size 2) of int(1..3)` could have valid values like `{1,2}`, `{1,3}`, `{2,3}`. The 'inner' domain is `int(1..3)`, from which the values that make up the set are pulled.

The most common attribute is cardinality (which restricts the range objects in an object), but they are type-specific and there is a large variety.


## Representation preferences
Any domain can carry a *representation preference*: the short name of the representation a declaration should be given, rather than leaving the choice to the heuristic. It is stored on the domain node it was written on and is not inherited by the domains inside it, so `matrix (representation components) indexed by [int(1..3)] of int(representation order, 1..4)` pins the matrix layout and, separately, the encoding of its entries.

Where it lives depends on the domain:

* `int`, `tuple`, `record`, `variant` and `matrix` carry it as an `Option<String>` field on their [`GroundDomain`] and [`UnresolvedDomain`] variants.
* `set`, `mset`, `sequence`, `function`, `relation`, `partition` and `permutation` carry it in their attribute struct (`SetAttr::representation` and so on).

[`Domain::representation_preference`] reads it, [`Domain::set_representation_preference`] writes it and [`Domain::has_representation_preference`] looks for one anywhere in a domain tree.

In Essence it is the attribute `representation <name>`, written just after the first word of the domain. `int` and `tuple` already use their parentheses for ranges and members, so it goes first inside them (`int(representation order, 1..4)`, `tuple (representation packed, int(1..3), bool)`); `matrix`, `record` and `variant` take it as their only attribute (`record (representation packed) {...}`); every other domain adds it to its attribute list (`function (representation explicit, total) ...`). `Display` and the grammar agree, so a printed domain parses back to an equal one. The interactive heuristic uses this to print each option as a declaration you can paste into the model.

A preference is honoured when that representation is applicable to the declaration; otherwise selection falls back to the heuristic, exactly as for sets.

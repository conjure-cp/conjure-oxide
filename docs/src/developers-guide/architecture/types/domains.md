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
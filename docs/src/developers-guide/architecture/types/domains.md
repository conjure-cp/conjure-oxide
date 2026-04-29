# Domains

## What are Domains
Domains are essentially some collection of values that some type can inherit. 

In the Essence language grammar, 'domain' is used both to describe high level 'types' (e.g. sets, matrices, sequences) and the collection inner atomic literals (i.e. booleans and integers). You can picture it in a recursive sense, where the atomic literals are the base case, and complex high-level types are domains composed of domains. 

In the orginal language decription, found in the [Conjure Docs](https://conjure.readthedocs.io/en/latest/essence.html)
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


## Domains in Essence and Conjure-Oxide
Looking at `conjure-cp-core::ast::domains`, there is a `Domain` Enum, with variants `Ground` and `Unresolved`. These themselves are both Enums that can be any one of the many complex types or simple types like booleans, integers, and the empty domain. The variants also have values, with the complex types often containing attribute structs and some 'inner' domain from which the complex type pulls it's values. 

> When defining a Set type, it may have attributes and an inner domain. For example, the domain `set (size 2) of int(1..3)` could have valid values like `{1,2}`, `{1,3}`, `{2,3}`. The 'inner' domain is `int(1..3)`, from which the values that make up the set are pulled.

The `Domain` enum (which must either be `Ground` or `Unresolved`) has it's value wrapped in a `ast::Moo` (titled as a pun on Cow, or Clone-on-Write) which ensures that clones of the domain that are owned by other functions do not modify the original if they are written to.

## Ground and Unresolved
* A ground domain is a domain that has set values. For example: `int(2..5)`, `int(1, 5..8, 9)`, or `int(1..)`.
* An unresolved domain is a domain that does not have set values. For example, `int(1..n)`. 


## Attributes
Domain attributes add a lot of the generality and power to Essence that simpler languages lack. In Essence Prime (a.k.a Essence'), if you wanted to capture the concept that some set has a variable size, you must have a large **fixed**-size collection wherein as the size varies, there are some slots that are tagged as out-of-bounds. In Essence, you can define a set domain `set (minSize 3, maxSize 5) ...` which handles this for you. At the moment, this gets translated as described above whenever transpiling down to a solver-specific language - but hopefully with this richer level of information, we would be able to make better inference and therefore have a better translation.
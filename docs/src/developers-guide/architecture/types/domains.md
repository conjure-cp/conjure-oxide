# Domains

## What are Domains
Domains are essentially some collection of values that some type can inherit. In maths, there are a few very common domains such as the set of natural numbers or the empty set. 

The word 'domain' is _seemingly_ used in multiple different contexts; in the original [Conjure docs](https://conjure.readthedocs.io/en/latest/essence.html), 'domain' is both used to describe high level 'types' (e.g. sets, matrices, sequences) and the collection inner atomic literals (i.e. booleans and integers). The trick is that a domain is both, and recursive in a sense where the atomic literals are the base case, and complex high-level types are domains composed of domains. 

## Domains in Essence and Conjure-Oxide
Looking at `conjure-cp-core::ast::domains`, you can see that a `Domain` is an Enum, that is either `Ground` or `Unresolved` (this is discussed in more detail below). These themselves are both Enums that can be any one of the many complex types or simple types like booleans, integers, and the empty domain. The variants also have values, with the complex types often containing attribute structs and some 'inner' domain from which the complex type pulls it's values. 

> When defining a Set type, it may have attributes and an inner domain. For example, the domain `set (size 2) of int(1..3)` could have valid values like `{1,2}`, `{1,3}`, `{2,3}`. The 'inner' domain is `int(1..3)`, from which the values that make up the set are pulled.

The `Domain` enum (which must either be `Ground` or `Unresolved`) has it's value wrapped in a `ast::Moo` (titled as a pun on Cow, or Clone-on-Write) which ensures the Domain

## Ground and Unresolved
A ground domain is a domain that has set values. For example:
```
int(2..5)
int(1, 5..8, 9)
int(1..)
```

An unresolved domain is a domain that does not have set values. For example, `int(1..n)`. 


## Attributes
Domain attributes add a lot of the generality and power to Essence that simpler languages lack. In Essence Prime (a.k.a Essence'), if you wanted to capture the concept that some set has a variable size, you must have a large fixed-size collection wherein as the size varies, there are some slots that are tagged as out-of-bounds. In Essence, you can define a set domain `set (minSize 3, maxSize 5) ...` which handles this for you. This `size` field is an attribute. 
# Tuples

A tuple is a collection of values of different domains.

## Tuple Domain

A tuple can be explicitly declared using the `tuple` specifier.

```essence
given a: tuple (bool, int(0..5), bool)
given b: tuple (int, tuple (bool, bool))
given c: tuple (int)
```

For tuples of at least two elements, a shorthand syntax can be used:

```
given a: (bool, int(0..5), bool)
given b: (int, (bool, bool))
```

Tuples can contain any domain, including other tuples.

### Attributes

Tuples take no attributes.

## Tuple Literal

Tuple literals can be declared in the same explicit fashion...

```essence
letting a be tuple (true, 4, false)
letting b be tuple (10, tuple (true, true))
letting c be tuple (9)
```

...or using the shorthand if there are at least two elements.

```
letting a be (true, 4, false)
letting b be (10, (true, true))
```

## Tuple Operators

TODO

## BNF

```bnf
<TupleDomain> ::= "tuple" "(" list(<Domain>) ")"
                | "(" list(<Domain>, min=2) ")"

<TupleLiteral> ::= "tuple" "(" [list(<Expression>)] ")"
                 | "(" list(<Expression>, min=2) ")"
```

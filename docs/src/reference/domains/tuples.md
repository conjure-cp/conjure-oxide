# Tuples

A tuple is a collection of values of different domains.

## Tuple Domain

```bnf
<TupleDomain> ::= "tuple" "(" list(<Domain>) ")"
                | "(" list(<Domain>, min=2) ")"
```

```essence
$ explicit syntax:
given a: tuple (bool, int(0..5), bool)
given b: tuple (int, tuple (bool, bool))
given c: tuple (int)

$ implicit syntax:
given a: (bool, int(0..5), bool)
given b: (int, (bool, bool))
```

### Attributes

Tuples take no attributes.

## Tuple Literal

```bnf
<TupleLiteral> ::= "tuple" "(" [list(<Expression>)] ")"
                 | "(" list(<Expression>, min=2) ")"
```

```essence
$ explicit syntax:
letting a be tuple (true, 4, false)
letting b be tuple (10, tuple (true, true))
letting c be tuple (9)

$ implicit syntax:
letting a be (true, 4, false)
letting b be (10, (true, true))
```

## Tuple Operators

TODO

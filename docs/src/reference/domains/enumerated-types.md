# Enumerated Types

Enumerated types (enums) consist of a set of named values (members).

Enum members are ordered based on the order they are defined.

## Enumerated Type Declaration

See [enumerated type declaration](../statements/letting.md#enumerated-type-declaration) in the `letting` reference.

```essence
letting direction be new type enum {North, East, South, West}
find x,y : direction
such that x != y
```

## Referencing Enum Members

Once an enum is declared, all enum members can be referenced by their own identifier:

```essence
letting cardinality be new type enum {North, East, South, West}
letting a be North
letting b be South
```

## Enumerated Type Operations

### `pred`

```rs
pred(m: member of enum E) -> member of enum E
```

Returns the member of enum `E` immediately preceding `m`.

_Example:_

```essence
letting E be new type enum {First, Second, Third}
find a such that a = pred(Second)
$ a = First
```

### `succ`

```rs
succ(m: member of enum E) -> member of enum E
```

Returns the member of enum `E` immediately succeeding `m`.

_Example:_

```essence
letting E be new type enum {First, Second, Third}
find a such that a = succ(Second)
$ a = Third
```

# Partitions

A partition is a datatype that splits some domain into parts.

## Partition Domains

Partitions are declared using the `partition` specifier.

```essence
given a: partition from int(0..10)
given b: partition (partSize 5, regular) from int(-10..20)
```

### Partition Attributes

Certain attributes control the number of parts a partition may have.

| Attribute     | Domain     | Description                                   |
| ------------- | ---------- | --------------------------------------------- |
| `numParts`    | `int(0..)` | The exact number of parts in the partition.   |
| `minNumParts` | `int(0..)` | The minimum number of parts in the partition. |
| `maxNumParts` | `int(0..)` | The maximum number of parts in the partition. |

Attributes may also define the size of the parts.

| Attribute     | Domain     | Description                                                                |
| ------------- | ---------- | -------------------------------------------------------------------------- |
| `partSize`    | `int(0..)` | The exact cardinality (number of elements) of each part.                   |
| `minPartSize` | `int(0..)` | The minimum cardinality of each part.                                      |
| `maxPartSize` | `int(0..)` | The maximum cardinality of each part.                                      |
| `regular`     | `bool`     | If set, every part must have the same cardinality (i.e. be the same size). |

## Partition Literals

A partition consists of a list of parts, each of which is a set literal.

```essence
letting a be partition({1, 2}, {3}, {4, 5, 6}, {})
```

## Partition Operations

### `together`

```rs
together(s: set of T, p: partition from T): bool
```

Returns `true` if a list of elements are all in the same part of the partition.

```essence
letting x be partition({1, 2}, {3}, {4, 5, 6}, {})
letting a be together({4, 6}, x)
$ a = true
```

### `apart`

```rs
apart(s: set of T, p: partition from T): bool
```

Returns `true` if a list of elements are _not_ all contained in one part of the partition.

```essence
letting x be partition({1, 2}, {3}, {4, 5, 6}, {})
letting a be apart({1, 4, 6}, x)
$ a = true
```

### `party`

```rs
party(e: T, p: partition of T): set of T
```

Returns the part of partition (as a set) that contains the specified element.

```essence
letting x be partition({1, 2}, {3}, {4, 5, 6}, {})
letting a be party(1, x)
$ a = {1, 2}
```

### `parts`

```rs
parts(p: partition from T): set of (set of T);
```

Takes a partition and returns its set of parts.

```essence
letting x be partition({1, 2}, {3}, {4, 5, 6}, {})
letting a be parts(x)
$ a = {{1, 2}, {3}, {4, 5, 6}, {}}
```

### `participants`

```rs
participants(p: partition from T): set of T;
```

Returns the union of all parts of a partition.

```essence
letting x be partition({1, 2}, {3}, {4, 5, 6}, {})
letting a be participants(x)
$ a = {1, 2, 3, 4, 5, 6}
```

## BNF

```bnf
<PartitionDomain> ::= "partition" ["(" list(<PartitionAttribute>) ")"] "from" <Domain>
<PartitionAttribute> ::= "numParts" <Expression.Integer>
                       | "minNumParts" <Expression.Integer>
                       | "maxNumParts" <Expression.Integer>
                       | "partSize" <Expression.Integer>
                       | "minPartSize" <Expression.Integer>
                       | "maxPartSize" <Expression.Integer>
                       | "regular"

<PartitionLiteral> ::= "partition" "(" [list(<SetLiteral>)] ")"
```

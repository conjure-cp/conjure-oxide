# Partitions

A partition is a datatype that splits some domain into parts.

## Partition Domains

```bnf
; QUESTION: should accept expressions? argue yes.
<PartitionDomain> ::= "partition" ["(" list(<PartitionAttribute>) ")"] "from" <Domain>
<PartitionAttribute> ::= "numParts" <Expression.Integer>
                       | "minNumParts" <Expression.Integer>
                       | "maxNumParts" <Expression.Integer>
                       | "partSize" <Expression.Integer>
                       | "minPartSize" <Expression.Integer>
                       | "maxPartSize" <Expression.Integer>
                       | "regular"
```

```essence
letting a be domain partition (partSize 5, regular) from int(0..10)
```

### Attributes

| Attribute     | Domain     | Description                                   |
| ------------- | ---------- | --------------------------------------------- |
| `numParts`    | `int(0..)` | The exact number of parts in the partition.   |
| `minNumParts` | `int(0..)` | The minimum number of parts in the partition. |
| `maxNumParts` | `int(0..)` | The maximum number of parts in the partition. |

| Attribute     | Domain     | Description                                              |
| ------------- | ---------- | -------------------------------------------------------- |
| `partSize`    | `int(0..)` | The exact cardinality (number of elements) of each part. |
| `minPartSize` | `int(0..)` | The minimum cardinality of each part.                    |
| `maxPartSize` | `int(0..)` | The maximum cardinality of each part.                    |

| Attribute | Domain | Description                                                                |
| --------- | ------ | -------------------------------------------------------------------------- |
| `regular` | `bool` | If set, every part must have the same cardinality (i.e. be the same size). |

## Partition Literals

```bnf
<PartitionLiteral> ::= "partition" "(" [list(<SetLiteral>)] ")"
```

```essence
letting a be partition({1, 2}, {3}, {4, 5, 6})
```

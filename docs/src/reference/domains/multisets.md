# Multisets

A multiset is a datatype for storing a set of objects where objects can occur more than once, but the ordering of objects does not matter.

## Multiset Domains

```bnf
<MultiSetDomain> ::= "mset" ["(" list(<MultiSetAttribute>) ")"] "of" <Domain>
<MultiSetAttribute> ::= "size" <Expression.Integer>
                      | "minSize" <Expression.Integer>
                      | "maxSize" <Expression.Integer>
                      | "minOccur" <Expression.Integer>
                      | "maxOccur" <Expression.Integer>
                      | "representation" <Identifier>
```

### Multiset Attributes

| Attribute | Domain     | Description                                               |
| --------- | ---------- | --------------------------------------------------------- |
| `size`    | `int(0..)` | The exact number of elements contained by the multiset.   |
| `minSize` | `int(0..)` | The minimum number of elements contained by the multiset. |
| `maxSize` | `int(0..)` | The maximum number of elements contained by the multiset. |

A range of sizes may be specified using `minSize x maxSize y`. Using contradicting values for the attribute values may result in an empty domain.

| Attribute  | Domain     | Description                            |
| ---------- | ---------- | -------------------------------------- |
| `minOccur` | `int(0..)` | The minimum number of occurrences TODO |
| `maxOccur` | `int(0..)` | The maximum number of occurrences TODO |

## Multiset Literals

```bnf
<MultiSetLiteral> ::= "mset" "(" [list(<Expression>)] ")"
```

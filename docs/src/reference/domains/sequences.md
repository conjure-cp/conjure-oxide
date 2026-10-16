# Sequences

A sequence is a datatype for storing a series of values, where values can occur multiple times and ordering is preserved.

## Sequence Domains

```bnf
<SequenceDomain> ::= "sequence" ["(" list(<SequenceAttribute>) ")"] "of" <Domain>
```

Unlike [matrices], sequences can have defined attributes; i.e. you can define a sequence to have varying lengths, and to restrict what values it can draw from it's inner domain.

### Attributes

```bnf
<SequenceAttribute> ::= "size" <Expression.Integer>
                      | "minSize" <Expression.Integer>
                      | "maxSize" <Expression.Integer>
                      | "injective"
                      | "surjective"
                      | "bijective"
```

#### Cardinality

| Attribute | Domain     | Description                                                  |
| --------- | ---------- | ------------------------------------------------------------ |
| `size`    | `int(0..)` | the exact cardinality (number of elements) of the sequence   |
| `minSize` | `int(0..)` | the minimum cardinality (number of elements) of the sequence |
| `maxSize` | `int(0..)` | the maximum cardinality (number of elements) of the sequence |

Sequences **must** have either a `size` or `maxSize` cardinality attribute.

#### In/Sur/Bijectivity

| Attribute    | Domain | Description   |
| ------------ | ------ | ------------- |
| `injective`  | `bool` | If set, TODO. |
| `surjective` | `bool` | If set, TODO. |
| `bijective`  | `bool` | If set, TODO. |

Using contradicting values for the attribute values may result in an empty domain.

## Sequence Literals

```bnf
<SequenceLiteral> ::= "sequence" "(" [list(<Expression>)] ")"
```

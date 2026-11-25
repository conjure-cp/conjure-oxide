# Sequences

A sequence is a datatype for storing a series of values, where values can occur multiple times and ordering is preserved.

Unlike [matrices](./matrices.md), sequences can have defined attributes; i.e. you can define a sequence to have varying lengths, and to restrict what values it can draw from it's inner domain.

## Sequence Domains

Sequences are declared using the `sequence` specifier.

```essence
given a: sequence (size 10) of int
given b: sequence (minSize 3, surjective) of int(-5..5)
```

### Sequence Attributes

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

Sequence literals simply list their values in order.

```essence
letting a be sequence (2, 3, 5, 7, 11, 13, 17, 19)
letting b be sequence (true, false, true, true, false, false)
```

## Sequence Operations

### `substring`

```essence
TODO
```

### `subsequence`

```essence
TODO
```

## BNF

```bnf
<SequenceDomain> ::= "sequence" ["(" list(<SequenceAttribute>) ")"] "of" <Domain>
<SequenceAttribute> ::= "size" <Expression.Integer>
                      | "minSize" <Expression.Integer>
                      | "maxSize" <Expression.Integer>
                      | "injective"
                      | "surjective"
                      | "bijective"

<SequenceLiteral> ::= "sequence" "(" [list(<Expression>)] ")"

<SequenceComparison> ::= <Expression.Sequence> ("substring"
                                              | "subsequence") <Expression.Sequence>
```

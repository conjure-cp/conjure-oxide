# Sets

## Set Domains

```bnf
<SetDomain> ::= "set" ["(" list(<SetAttribute>) ")"] "of" <Domain>
<SetAttribute> ::= "size" <Expression.Integer>
                 | "minSize" <Expression.Integer>
                 | "maxSize" <Expression.Integer>
                 | "representation" <Identifier>
```

Using contradicting values for the attribute values may result in an empty domain.

## Set Literals

```bnf
<SetLiteral> ::= "{" [list(<Expression>)] "}"
```

## Set Operations

### Union / Intersect

```bnf
<SetOperation> ::= <Expression.Set> ("union" | "intersect") <Expression.Set>
```

| Operation       | Description                                                                    |
| --------------- | ------------------------------------------------------------------------------ |
| `a union b`     | Returns a set containing the elements contained in either `a` or `b`, or both. |
| `a intersect b` | Returns a set containing the elements contained in both `a` and `b`.           |

### Comparison

```bnf
<InExpression> ::= <Expression> "in" <Expression.Set>
<SetComparisonExpression> ::= <Expression.Set> "subset"
                                    | "subsetEq"
                                    | "supset"
                                    | "supsetEq") <Expression.Set>
```

| Operation      | Description                                                                                               |
| -------------- | --------------------------------------------------------------------------------------------------------- |
| `a in b`       | Returns `true` if element `a` is a member of set `b`.                                                     |
| `a subset b`   | Returns `true` if set `a` is a proper subset of set `b` (all elements of `a` are in `b`, but `a != b`).   |
| `a subsetEq b` | Returns `true` if set `a` is a subset of set `b` (all elements of `a` are in `b`).                        |
| `a supset b`   | Returns `true` if set `a` is a proper superset of set `b` (all elements of `b` are in `a`, but `a != b`). |
| `a supsetEq b` | Returns `true` if set `a` is a superset of set `b` (all elements of `b` are in `a`).                      |

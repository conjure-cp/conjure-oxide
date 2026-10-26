# Collection Operations

```bnf
<SetOperation> ::= <Expression.Set> ("union" | "intersect") <Expression.Set>
```

```bnf
<SetComparison> ::= <Expression.Set> ("in"
                                    | "subset"
                                    | "subsetEq"
                                    | "supset"
                                    | "supsetEq") <Expression.Set>
```

```bnf
<SequenceComparison> ::= <Expression.Sequence> ("substring"
                                              | "subsequence") <Expression.Sequence>
```

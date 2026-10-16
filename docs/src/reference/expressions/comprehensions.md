# Comprehensions

```bnf
<Comprehension> ::= "[" <Expression> "|" list(<Generator>
                                            | <Condition>
                                            | <ComprehensionLetting>) "]"

<ComprehensionLetting> ::= "letting" <Identifier> "be" <Expression>

<ComprehensionGenerator> ::= (<Identifier> | <Tuple>) ":" <Domain>
                           | (<Identifier> | <Tuple>) "<-" (<Expression.Integer>
                                                          | <Identifier>
                                                          | <Matrix>
                                                          | <SetOperation>)
```

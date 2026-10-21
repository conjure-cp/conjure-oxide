# Permutations

## Permutation Domains

```bnf
<PermutationDomain> ::= "permutation" ["(" list(<PermutationAttribute>) ")"] "of" <Domain>
<PermutationAttribute> ::= "numMoved" <Expression.Integer>
                         | "minNumMoved" <Expression.Integer>
                         | "maxNumMoved" <Expression.Integer>
```

## Permutation Literals

```bnf
<PermutationLiteral> ::= "permutation" "(" [list(<PermutationLiteralCycle>)] ")"
<PermutationLiteralCycle> ::= "(" list(<Expression>) ")"
```

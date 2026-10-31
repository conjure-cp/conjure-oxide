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

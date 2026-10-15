# Records

## Record Domains

```bnf
<RecordDomain> ::= "record" "{" list(<NameDomainPair>) "}"
<NameDomainPair> ::= <Identifier> ":" <Domain>
```

## Record Literals

```bnf
<NameValuePair> ::= <Identifier> "=" <Expression>
<RecordLiteral> ::= "record" "{" list(<NameValuePair>) "}"
```

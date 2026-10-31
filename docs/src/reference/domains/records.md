# Records

A record is an unordered TODO list of key-value pairs.

A record may not have duplicate keys.

Other languages may refer to this concept as a "struct" (C, C++, Rust, etc.), "dataclass" (Python), or otherwise.

## Record Domains

```bnf
<RecordDomain> ::= "record" "{" list(<NameDomainPair>) "}"
<NameDomainPair> ::= <Identifier> ":" <Domain>
```

```essence
letting a be domain record {}
letting b be domain record { A : int(0..1), B : int(0..2) }
```

## Record Literals

```bnf
<RecordLiteral> ::= "record" "{" list(<NameValuePair>) "}"
<NameValuePair> ::= <Identifier> "=" <Expression>
```

```essence
letting a be record { }
letting b be record { A = 1, B = 0 }
```

# Records

A record is an unordered list of key-value pairs.

A record may not have duplicate keys.

Other languages may refer to this concept as a "struct" (C, C++, Rust, etc.), "dataclass" (Python), or otherwise.

## Record Domains

Records are declared using the `record` specifier, and then declaring a list of name-domain pairs.

```essence
letting a be domain record {}
letting b be domain record { A : int(0..1), B : int(0..2), C: set of bool }
```

### Record Attributes

Records take no attributes.

## Record Literals

A record literal gives concrete values to each field in the record domain.

```essence
letting a be record { }
letting b be record { A = 1, B = 0, C = {true, false} }
```

## BNF

```bnf
<RecordDomain> ::= "record" "{" list(<NameDomainPair>) "}"
<NameDomainPair> ::= <Identifier> ":" <Domain>

<RecordLiteral> ::= "record" "{" list(<NameValuePair>) "}"
<NameValuePair> ::= <Identifier> "=" <Expression>
```

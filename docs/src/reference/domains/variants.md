# Variants

A variant domain stores named domain pair fields.

However, unlike a [[record TODO]], a variant literal can only use one of these fields at once. This field is referred to as **active**.

## Variant Domains

```bnf
<VariantDomain> ::= "variant" "{" list(<NameDomainPair>) "}"
<NameDomainPair> ::= <Identifier> ":" <Domain>
```

```essence
given a: variant { first: bool, second: int(0..10), third: set of int }
```

### Attributes

Variants take no attributes.

## Variant Literals

```bnf
<VariantLiteral> ::= "variant" "{" <NameValuePair> "}"
<NameValuePair> ::= <Identifier> ":" <Expression>
```

```essence
letting a be variant { second: 5 }
```

## Operators

### active

```rs
active(v: variant, f: identifier): bool;
```

Returns true if field `f` is currently the field active in variant `v`.

```essence
letting V be domain variant { first: bool, second: int(0..10), third: set of int }
letting a: V be variant { second: 5 }

letting is_first_active be active(a, first)
$ is_first_active = false

letting is_second_active be active(a, second)
$ is_second_active = true
```

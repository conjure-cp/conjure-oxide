# Variants

A variant domain stores named domain pair fields.

However, unlike a [record](./records.md), a variant literal can only use _one of these fields at once_. This field is referred to as **active**.

## Variant Domains

Variants are declared using the `variant` specifier, followed by the list of possible active fields.

```essence
given a: variant { first: bool, second: int(0..10), third: set of int }
```

### Attributes

Variants take no attributes.

## Variant Literals

Defining a variant literal involves specifying its active field.

```essence
letting a be variant { second: 5 }
```

However, note that this type has no non-active variants, unless they are specified by a domain annotation:

```essence
letting A be domain variant { first: bool, second: int(0..10), third: set of int }
letting a: A be variant { second: 5 }
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

## BNF

```bnf
<VariantDomain> ::= "variant" "{" list(<NameDomainPair>) "}"
<NameDomainPair> ::= <Identifier> ":" <Domain>

<VariantLiteral> ::= "variant" "{" <NameValuePair> "}"
<NameValuePair> ::= <Identifier> ":" <Expression>
```

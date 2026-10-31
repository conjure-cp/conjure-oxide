# Variants

A variant domain stores named domain pair fields.

However, unlike a [[record TODO]], a variant literal can only use one of these fields at once. This field is referred to as **active**.

## Variant Domains

```
<VariantDomain> ::= "variant" "{" list(<NameDomainPair>) "}"
<NameDomainPair> ::= <Identifier> ":" <Domain>
```

### Attributes

Variants take no attributes.

## Variant Literals

```bnf
<VariantLiteral> ::= "variant" "{" <NameValuePair> "}"
```

## Operators

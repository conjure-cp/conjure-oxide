# Letting Statements

Letting statements assign a value or domain to an [identifier](/docs/src/reference/identifiers.md).

```bnf
<LettingStatement> ::= "letting" list(<LettingVariableDeclaration>)
<LettingVariableDeclaration> ::= list(<Identifier>) "be" (<Expression>
                                                        | <DomainDeclaration>
                                                        | <EnumDeclaration>
                                                        | <UnnamedTypeDeclaration>)

```

## Value Declaration

```essence
letting x be y + z
letting a, b be 1    $ a <- 1, b <- 1
```

## Domain Declaration

```bnf
<DomainDeclaration> ::= "domain" <Domain>
```

```essence
letting d be domain set of int(a..b)
```

## Enumerated Type Declaration

```bnf
<EnumDeclaration> ::= "new type enum" "{" list(<Identifier>) "}"
```

The given-enum syntax defers the specification of actual values of the enumerated type until instantiation (which can be in a parameter file).

```essence
letting direction be new type enum {North, East, South, West}
find x,y : direction
such that x != y
```

<!-- TODO: Enumerated types support equality, ordering, and successor/predecessor operators; they do not support arithmetic operators -->

## Unnamed Type Declaration

```bnf
<UnnamedTypeDeclaration> ::= "new type of size" <Expression>
```

The members of an unnamed type cannot be referred to individually. Typically constraints are posted using quantified variables over the whole domain.

```essence
letting a be new type of size 5
```

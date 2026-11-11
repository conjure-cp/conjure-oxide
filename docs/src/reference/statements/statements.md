# Statements

```bnf
<Statement> ::= <DeclarationStatement>
              | <SuchThatStatement>
              | <WhereStatement>
              | <ObjectiveStatement>
              | <BranchingStatement>
```

## Declarations

```bnf
<DeclarationStatement> ::= <FindStatement>
                         | <GivenStatement>
                         | <LettingStatement>
```

## Statement Order

Every symbol must be declared before it is used, but otherwise statements can be listed in any order.

# Find Statements

<!-- more general than Essence docs (findAux?) -->

```bnf
<FindStatement> ::= ("findAux" | "find") list(<VariableDeclaration>)
<VariableDeclaration> ::= list(<Identifier>) (":" <Domain> | "new type enum")
```

The domain of a decision variable must be finite.

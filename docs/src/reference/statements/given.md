# Given Statements

```bnf
<GivenStatement> ::= "given" list(<VariableDeclaration>)
```

The domain of a parameter can be finite or infinite.

```essence
$ in problem specification
given x: int

$ in parameter file
letting x be 5
```

# Where Statements

```bnf
<WhereStatement> ::= "where" list(<Expression.Boolean>)
```

Where statements cannot refer to decision variables.
They can be used to post conditions on the parameters of the problem specification.

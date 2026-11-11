# Branching Statements

```bnf
; TODO: not included in Essence docs
<BranchingStatement> ::= "branching" "on" "[" list(<Identifier> | <Expression.Boolean>) "]"
```

Specifies the search order during constraint satisfaction.

Given the below branching statement, the solver will first branch on `x`, then `y`, then `z`.

```essence
branching on [x,y,z]
```

Branching order may also be specified in terms of boolean expressions.

```essence
find solution: set of int(0..10)
$ first branch on solutions containing 3, then 5.
branching on [3 in solution, 5 in solution]
```

A problem specification can contain **at most one** branching statement.

TODO differentiating between boolean expressions and identifiers?
TODO mixing identifiers and booleans?
TODO pareto?

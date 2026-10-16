# Dominance Relation Statements

```bnf
<ObjectiveStatement> ::= ("minimising" | "maximising") <Expression.Integer>
```

Objective statements transform the problem into an optimisation problem.

A problem specification can contain at most one objective statement.

```bnf
<DominanceRelation> ::= "dominance relation" <Expression.Boolean>
```

(From Book) At a high level:

- Solve the model and obtain a solution.
- Add a new constraint that blocks any future solution dominated by that solution.
- Continue until no more solutions remain (or the user stops search).

## Dominance Relations

TODO: not included in Essence docs

```bnf
<ParetoExpression> ::= "pareto" "(" list(<ParetoItem>) ")"
<ParetoItem> ::= ("minimising" | "maximising") <Expression>
```

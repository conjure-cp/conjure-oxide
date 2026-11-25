# Statements

A statement is a unit of code which modifies the constraint model in some way. This may be:

- Adding a new decision variable
- Declaring a new concrete value
- Adding a constraint

## Declarations

A declaration gives meaning to an identifier.

- [`letting`](./letting.md)
- [`given`](./given.md)
- [`find`](./find.md)

### Statement Order

Every symbol must be declared before it is used, but otherwise statements can be listed in any order.

## Constraints

A constraint restricts the solution space by requiring some condition be met.

- [`where`](./where.md)
- [`such that`](./such-that.md)
- [`dominance relation`](./dominance-relations.md)

## BNF

```bnf
<Statement> ::= <DeclarationStatement>
              | <SuchThatStatement>
              | <WhereStatement>
              | <ObjectiveStatement>
              | <BranchingStatement>

<DeclarationStatement> ::= <FindStatement>
                         | <GivenStatement>
                         | <LettingStatement>
```

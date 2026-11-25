# Domains

A domain is the discrete and finite set of values that a variable or expression can take.

An instantiation of a domain is known as a **literal**. A literal is a representation of _one_ specific value of a domain.

```bnf

```

| Type     | Domain                                 | Literal                      |
| -------- | -------------------------------------- | ---------------------------- |
| Boolean  | `bool`                                 | `true`, `false`              |
| Integer  | `int`, `int(0..5)`                     | `-5`, `0`, `3`               |
| Tuple    | `(int, bool, bool)`                    | `(1, false, true)`           |
| Matrix   | `matrix indexed by [int, int] of bool` | `[true, false, true, false]` |
| Function | `function int --> set of int`          | `function`                  |
| Record   | `record (`                             | `[true, false, true, false]` |

## BNF

```bnf
<Domain> ::= <Identifier>
           | <BoolDomain>
           | <IntDomain>
           | <TupleDomain>
           | <MatrixDomain>
           | <RecordDomain>
           | <VariantDomain>
           | <SetDomain>
           | <MsetDomain>
           | <SequenceDomain>
           | <FunctionDomain>
           | <RelationDomain>
           | <PartitionDomain>
           | <PermutationDomain>

<Literal> ::= <Constant>
            | <SetLiteral>
            | <MultiSetLiteral>
            | <SequenceLiteral>
            | <TupleLiteral>
            | <MatrixLiteral>
            | <FunctionLiteral>
            | <RelationLiteral>
            | <PartitionLiteral>
            | <PermutationLiteral>
            | <RecordLiteral>
            | <VariantLiteral>
```

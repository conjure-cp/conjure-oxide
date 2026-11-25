# Expressions

An expression is a unit of code which resolves to a _value_. This may either be:

- A value itself (e.g. `1`, `true`, `(1, 2, 3)`)
- A reference to a value (e.g. `a`, `b`)
- The computation of a value (e.g. `2 + 2`, `min([1, 2, 3])`)

## BNF

```bnf
<Expression> ::= <Literal>
               | <SubExpression>
               | <Comprehension>
               | <BoolExpr>
               | <ArithmeticExpr>
               | <ComparisonExpr>
               | <AnnotationExpr>
               | <FunctionalExpr>
               | <FunctionCallExpr>
```

```bnf
<SubExpression> ::= "(" <Expression> ")"
```

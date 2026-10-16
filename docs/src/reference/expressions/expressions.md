## Expressions

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

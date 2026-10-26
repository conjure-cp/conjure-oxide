# Comparison Expressions

```bnf
<ComparisonExpr> ::= <ArithmeticComparison>
                   | <LexComparison>
                   | <EqualityComparison>
                   | <SetComparison>
                   | <SequenceComparison>
```

```bnf
<ArithmeticComparison> ::= <Expression.Integer> ("<=" | ">=" | "<" | ">") <Expression.Integer>
<LexComparison> ::= <Expression.Lexical> ("<lex" | "<=lex" | ">lex" | ">=lex") <Expression.Lexical>
```

; supported for all types

```bnf
<EqualityComparison> ::= <Expression> ("=" | "!=") <Expression>
```

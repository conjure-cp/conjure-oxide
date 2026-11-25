# Comparison Expressions

## Equality Comparison

Equality simply determines whether two values are **structurally** identical.

```essence
1 = 1                    $ true
{1, 2, 3} = {3, 2, 1}    $ true
(false, 1) = (false, 1)  $ true
2 + 2 = 5                $ false
```

`a != b` is `true` if `!(a = b)`.

## Arithmetic Comparison

The standard arithmetic comparators operate over integers.

```essence
a <= b
a >= b
a < b
a > b
```

## Lexicographic Comparison

Lexicographic comparisons (e.g. `<=lex`) operate over ordered collections.

TODO complete

## BNF

```bnf
<ComparisonExpr> ::= <ArithmeticComparison>
                   | <LexComparison>
                   | <EqualityComparison>
                   | <SetComparison>
                   | <SequenceComparison>

<ArithmeticComparison> ::= <Expression.Integer> ("<=" | ">=" | "<" | ">") <Expression.Integer>

<LexComparison> ::= <Expression.Lexical> ("<lex" | "<=lex" | ">lex" | ">=lex") <Expression.Lexical>

<EqualityComparison> ::= <Expression> ("=" | "!=") <Expression>
```

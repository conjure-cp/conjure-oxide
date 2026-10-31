# Arithmetic Expressions

```bnf
<ArithmeticExpr> ::= <BinaryArithExpr>
                    | <NegativeExpr>
                    | <AbsValueExpr>
                    | <FactorialExpr>


<BinaryArithExpr> = ::= <Expression.Integer> ("+" | "-" | "*" | "/" | "%" | "**") <Expression.Integer>
<NegativeExpr> ::= "-" <Expression.Integer>
<AbsValueExpr> ::= "|" <Expression.Integer> "|"
<FactorialExpr> ::= <Expression.Integer> "!"
                 | "factorial" "(" <Expression.Integer> ")"
```

## Addition

`a + b` returns the sum of `a` and `b`.

## Subtraction

`a - b` returns the result of subtracting `b` from `a`.

## Multiplication

`a * b` returns the product of `a` and `b`.

## Division

`a / b` returns the result of dividing `a` by `b`, **rounded down to the nearest integer**.

### Division by Zero

The value of `x / 0` is undefined for all `x`. Therefore,

TODO: what is supposed to happen with division by zero?

## Modulus

`a % b` returns the remainder of dividing `a` by `b` (e.g. `10 % 3` is `1`).

## Exponent

`a ** b` returns `a` to the power `b` (e.g. `2 ** 5` is `32`).

## Negation

`-a` returns the arithmetic negation of `a` (`a * -1`).

## Absolute

`|a|` returns the absolute value of `a` (i.e. its distance from the value zero).

## Factorial

`a!` or `factorial(a)` returns the factorial of `a` (i.e. the product of all integers from `1` to `a` inclusive).

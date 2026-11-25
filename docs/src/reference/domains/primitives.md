# Primitives

Essence supports two primitive domains: booleans and integers.

## Booleans

```essence
given a: bool
```

The Boolean domain has values `false` and `true`, ordered as such.

```essence
letting a be true
letting b be false
```

## Integers

Integer domains span over the whole numbers, and can be bounded by a list of ranges.

```bnf

```

A range is either a single integer, or a list of sequential integers with a given lower (inclusive) and upper (exclusive) bound.

```essence
given a: int            $ a is any integer
given b: int(..)        $ b is any integer
given c: int(4)         $ c = 4
given d: int(1, 3, 7)   $ d = 1 | d = 3 | d = 7
given e: int(-2..)      $ e >= -2
given f: int(-5..5)     $ -5 <= f < 5
given g: int(1, 3..6)   $ g = 1 | 3 <= g < 6
given h: int(x..y)      $ where x and y are integers
given i: int(6,2,8..10) $ ranges can be out-of-order
```

Integer domains are instantiated with values in their range.

```bnf

```

```essence
letting a be 100
letting b be 32
letting c be 4
$ etc...
```

## BNF

```bnf
<BoolDomain> ::= "bool"
<BooleanLiteral> ::= "true" | "false"

<IntDomain> ::= "int" ["(" [list(<IntRange>)] ")"]
<IntRange>  ::= <Expression.Integer>
              | [<Expression.Integer>] ".." [<Expression.Integer>]
<Integer> ::= `-?[0-9]+`
```

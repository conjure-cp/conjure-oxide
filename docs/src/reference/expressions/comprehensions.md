# Comprehensions

```bnf
<Comprehension> ::= "[" <Expression> "|" <ComprehensionGenerator> ["," list(<Generator>)] ["," list(<ComprehensionLettingOrCondition>)] "]"
```

Comprehensions allow the creation of lists via mapping and filtering of values in an existing list or domain.

## Generators

```bnf
<ComprehensionGenerator> ::= (<Identifier> | <Tuple>) ":" <Domain>
                           | (<Identifier> | <Tuple>) "<-" <Expression.List>
```

### List Generators

Comprehensions may iterate over a list to produce their output values.

The following generator takes the elements of `[1, 2, 3]`, and adds 1 to each:

```essence
letting a be [x + 1 | i <- [1, 2, 3]]
$ a = [2, 3, 4]
```

### Domain Generators

Comprehensions may also accept domains as the generator source using `:`.

The following comprehension `b` is equivalent to `a`:

```essence
letting b be [x + 1 | i : int(1..3)]
$ b = [2, 3, 4]
```

### Generator Value Unpacking

The comprehension expression may also unpack (aka. destructure) the value provided by the generator for the generator expression:

```essence
letting c be [u < v | (u,v) <- [(0,1), (4, 2), (8, 8)] ]
$ c = [true, false, false]

letting d be [x | (x, _) <- [(1, 4), (7, 9), (5, 3)] ]
$ d = [1, 7, 5]
```

## Conditions

```bnf
<ComprehensionLettingOrCondition> ::= <Expression.Boolean> | <ComprehensionLetting>
```

Comprehensions may also define conditions upon which the generator values are included in the output, listed after the generator expression.

```essence
$ returns the even integers from 1-10.
letting e be [x | x: int(1..10), x % 2 == 0]
$ e = [2, 4, 6, 8, 10]

$ returns the squares of integers 1-6, excluding 3^2 and 5^2.
letting f be [x * x | x: int(1..6), x != 3, x != 7]
$ f = [1, 4, 16, 36]
```

## Temporary Lettings

```bnf
<ComprehensionLetting> ::= "letting" <Identifier> "be" <Expression>
```

To aid readability, lettings can also be defined within the scope of comprehensions.

```essence
letting M be [5, 10, 32, -9]

$ returns a list containing i + M[i] for all i in domain I.
letting g be [x + y | i : int(0..3), letting x be i, letting y be M[i] ]
$ g = [5, 11, 34, -6]
```

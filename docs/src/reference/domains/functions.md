# Functions

A function is a binary relation on two sets, from elements of an input domain to elements of an output codomain.

## Function Domains

```essence
given a: function (injective) bool --> bool
given b: function (maxSize 10, surjective) int(0..) --> set of int
```

### Function Attributes

#### Size

| Attribute | Domain     | Description                                                       |
| --------- | ---------- | ----------------------------------------------------------------- |
| `size`    | `int(0..)` | The exact number of mappings for which the function is defined.   |
| `minSize` | `int(0..)` | The minimum number of mappings for which the function is defined. |
| `maxSize` | `int(0..)` | The maximum number of mappings for which the function is defined. |

A range of sizes may be specified using `minSize x, maxSize y`. Using contradicting values for the attribute values may result in an empty domain.

#### Partial vs Total

| Attribute | Domain | Description                                                                 |
| --------- | ------ | --------------------------------------------------------------------------- |
| `total`   | `bool` | If set, the function is defined for every possible input in the domain set. |

#### In/Sur/Bijectivity

| Attribute    | Domain | Description                                                                                                                |
| ------------ | ------ | -------------------------------------------------------------------------------------------------------------------------- |
| `injective`  | `bool` | If set, every element of the function's domain (input) maps to a unique element of its codomain (output).                  |
| `surjective` | `bool` | If set, every element of the function's codomain (output) maps from at least one element in the function's domain (input). |
| `bijective`  | `bool` | If set, the function is both injective and surjective.                                                                     |

## Function Literals

Instead of being defined programmatically (e.g. `f(x) = x + 1`), functions are defined by providing a list of mappings from inputs to outputs.

```essence
letting factors be function(1 --> {1}, 2 --> {1, 2}, 3 --> {1, 3}, 4 --> {1, 2, 4})

letting months be new type enum { ... }
letting daysInMonth be function (January -> 31, February -> 28, March -> 31, ...)
```

Functions may also supply more than one mapping per input:

```essence
letting sqrt be function(1 --> 1, 1 --> -1, 4 --> 2, 4 --> -2)
```

## Function Operations

### Function Application

User-defined functions can be applied just like built-in functions.

```essence
given myFunc: function int --> int
find b: int(1..10) = myFunc(3)
```

### `image`

```rs
image(f: function T --> V, x: T): V
```

Returns the application of function `f` to value `x` (i.e. `apply(f, x) = f(x)`).

```essence
given isOdd function: int --> bool
letting a be image(isOdd, 3)
$ a = true
```

### `imageSet`

```rs
imageSet(f: function T --> V, x: T): set of V
```

Applies function `f` to value `x`, returning the set of codomain values (i.e. set of possible output values of `f(x)`).

Returns the empty set if `f(x)` is not defined.

```essence
given sqrt function: int --> int
letting a be imageSet(sqrt, 4)
$ a = {2, -2}
```

### `preImage`

```rs
preImage(f: Function T --> V, x: V): set of T;
```

Applies the function `f` in reverse.

Returns the set containing every value `y` for which `f(y) = x`.

```essence
given addOne function: int --> int
letting a be preImage(addOne, 5)
$ a = 4
```

### `inverse`

```rs
inverse(f1: function T --> V, f2: function V --> T): bool
```

Returns true if functions `f1` and `f2` are the inverse of each other.

```essence
given addOne function: int --> int
given subtractOne function: int --> int
letting a be inverse(addOne, subtractOne)
$ a = true
```

### `defined`

```rs
defined(f: function T --> V): set of T
```

Returns the set of domain values for which function `f` is defined.

```essence
given openingHoursPerWeekday: int(1..7) --> set of int
letting a be defined(openingHoursPerWeekday)
$ a = {1, 2, 3, 4, 5, 6, 7}
```

### `range`

```rs
range(f: function T --> V): set of V
```

Returns the set of codomain values which function `f` can output.

```essence
given daysInMonth: int(1..12) --> int
letting a be range(daysInMonth)
$ a = {28, 30, 31}
```

### `restrict`

```rs
restrict(f: function T --> V, domain: domain in T): function T --> V;
```

Returns a sub-function of `f` which has its mapping restricted to values in `domain`.

```essence
given sqrt: int(1..40) --> int
letting sqrtReduced be restrict(sqrt, int(1..10))

letting a be sqrt(16)
$ a = 4
letting b be sqrtReduced(16)
$ b = [undefined!]
```

## BNF

```bnf
<FunctionDomain> ::= "function" ["(" list(<FunctionAttribute>) ")"] <Domain> "-->" <Domain>

<FunctionAttribute> ::= "size" <Expression.Integer>
                      | "minSize" <Expression.Integer>
                      | "maxSize" <Expression.Integer>
                      | "total"
                      | "injective"
                      | "surjective"
                      | "bijective"

<FunctionLiteral> ::= "function" "(" [list(<FunctionLiteralPair>)] ")"
<FunctionLiteralPair> ::= <Expression> "-->" <Expression>
```

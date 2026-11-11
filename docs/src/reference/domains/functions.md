# Functions

A function is a binary relation on two sets, from elements of an input domain to elements of an output codomain.

## Function Domains

```bnf
<FunctionDomain> ::= "function" ["(" list(<FunctionAttribute>) ")"] <Domain> "-->" <Domain>
```

```essence
given a: function (injective) bool --> bool
given b: function (maxSize 10, surjective) int(0..) --> set of int
```

### Function Attributes

```bnf
<FunctionAttribute> ::= "size" <Expression.Integer>
                      | "minSize" <Expression.Integer>
                      | "maxSize" <Expression.Integer>
                      | "total"
                      | "injective"
                      | "surjective"
                      | "bijective"
```

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

```bnf
<FunctionLiteral> ::= "function" "(" [list(<FunctionLiteralPair>)] ")"
<FunctionLiteralPair> ::= <Expression> "-->" <Expression>
```

## Function Operations

### Function Application

User-defined functions can be applied just like built-in functions.

```essence
given myFunc: function int -> int
find b: int(1..10) = myFunc(3)
```

### `image`

```rs
image(f: function T --> V, x: T): V
```

Returns the application of function `f` to value `x` (i.e. `apply(f, x) = f(x)`).

### `imageSet`

```rs
imageSet(f: function T --> V, x: T): set of V
```

Applies function `f` to value `x`, returning the set of codomain values (i.e. set of possible output values of `f(x)`).

Returns the empty set if `f(x)` is not defined.

### `preImage`

```rs
preImage(f: Function T --> V, x: V): set of T;
```

Applies the function `f` in reverse.

Returns the set containing every value `y` for which `f(y) = x`.

### `inverse`

```rs
inverse(f1: function T --> V, f2: function V --> T): bool
```

Returns true if functions `f1` and `f2` are the inverse of each other.

### `defined`

```rs
defined(f: function T --> V): set of T
```

Returns the set of domain values for which function `f` is defined.

### `range`

```rs
range(f: function T --> V): set of V
```

Returns the set of codomain values which function `f` can output.

### `restrict`

```rs
restrict(f: function T --> V, domain: domain in T): function T --> V;
```

Returns a sub-function of `f` which has its mapping restricted to values in `domain`.

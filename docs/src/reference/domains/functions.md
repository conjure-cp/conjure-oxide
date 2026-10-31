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

<!-- QUESTION: any constraints on possible types here? -->

```bnf
<FunctionLiteral> ::= "function" "(" [list(<FunctionLiteralPair>)] ")"
<FunctionLiteralPair> ::= <Expression> "-->" <Expression>
```

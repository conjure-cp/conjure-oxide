# Boolean Expressions

## Not Expression

`!a` is `true` iff `a` is `false`.

| `a`     | `!a`    |
| ------- | ------- |
| `true`  | `false` |
| `false` | `true`  |

## And Expression

`a /\ b` is `true` iff both `a` and `b` are `true`.

| `a`     | `b`     | `a /\ b` |
| ------- | ------- | -------- |
| `true`  | `true`  | `true`   |
| `true`  | `false` | `false`  |
| `false` | `true`  | `false`  |
| `false` | `false` | `false`  |

## Or Expression

`a \/ b` is `true` iff `a` is `true` or `b` is `true`.

| `a`     | `b`     | `a \/ b` |
| ------- | ------- | -------- |
| `true`  | `true`  | `true`   |
| `true`  | `false` | `true`   |
| `false` | `true`  | `true`   |
| `false` | `false` | `true`   |

## Implication Expression

`a -> b` is `true` iff `a` is `false` or `b` is `true`.

| `a`     | `b`     | `a -> b` |
| ------- | ------- | -------- |
| `true`  | `true`  | `true`   |
| `true`  | `false` | `false`  |
| `false` | `true`  | `true`   |
| `false` | `false` | `true`   |

## Iff Expression

`a <-> b` is `true` iff both `a` and `b` are `true`, or both `a` and `b` are `false`.

`a <-> b` is logically equivalent to `a = b`, but can **only** be applied to boolean expressions.

| `a`     | `b`     | `a <-> b` |
| ------- | ------- | --------- |
| `true`  | `true`  | `true`    |
| `true`  | `false` | `false`   |
| `false` | `true`  | `false`   |
| `false` | `false` | `true`    |

## BNF

```bnf
<BoolExpr> ::= <NotExpr>
| <AndExpr>
| <OrExpr>
| <ImplicationExpr>
| <IffExpr>
| <FunctionalAndExpr>
| <FunctionalOrExpr>
| <QuantifierExpr>

<NotExpr> ::= "!" <Expression.Boolean>

<AndExpr> ::= <Expression.Boolean> "/\" <Expression.Boolean>

<OrExpr> ::= <Expression.Boolean> "\/" <Expression.Boolean>

<ImplicationExpr> ::= <Expression.Boolean> "->" <Expression.Boolean>

<IffExpr> ::= <Expression.Boolean> "<->" <Expression.Boolean>
```

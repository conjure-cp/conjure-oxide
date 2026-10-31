# Boolean Expressions

```bnf
<BoolExpr> ::= <NotExpr>
| <AndExpr>
| <OrExpr>
| <ImplicationExpr>
| <IffExpr>
| <FunctionalAndExpr>
| <FunctionalOrExpr>
| <QuantifierExpr>
```

## Not Expression

```bnf
<NotExpr> ::= "!" <Expression.Boolean>
```

| `a`     | `!a`    |
| ------- | ------- |
| `true`  | `false` |
| `false` | `true`  |

## And Expression

```bnf
<AndExpr> ::= <Expression.Boolean> "/\" <Expression.Boolean>
```

`a /\ b` is `true` only if both `a` and `b` are true.

| `a`     | `b`     | `a /\ b` |
| ------- | ------- | -------- |
| `true`  | `true`  | `true`   |
| `true`  | `false` | `false`  |
| `false` | `true`  | `false`  |
| `false` | `false` | `false`  |

## Or Expression

```bnf
<OrExpr> ::= <Expression.Boolean> "\/" <Expression.Boolean>
```

`a \/ b` is `true` if `a` is `true` or `b` is `true`.

| `a`     | `b`     | `a \/ b` |
| ------- | ------- | -------- |
| `true`  | `true`  | `true`   |
| `true`  | `false` | `true`   |
| `false` | `true`  | `true`   |
| `false` | `false` | `true`   |

## Implication Expression

```bnf
<ImplicationExpr> ::= <Expression.Boolean> "->" <Expression.Boolean>
```

`a -> b` is `true` if `a` is `false` or `b` is `true`.

| `a`     | `b`     | `a -> b` |
| ------- | ------- | -------- |
| `true`  | `true`  | `true`   |
| `true`  | `false` | `false`  |
| `false` | `true`  | `true`   |
| `false` | `false` | `true`   |

## Iff Expression

```bnf
<IffExpr> ::= <Expression.Boolean> "<->" <Expression.Boolean>
```

`a <-> b` is `true` if both `a` and `b` are `true`, or both `a` and `b` are `false`.

`a <-> b` is logically equivalent to `a = b`, but can **only** be applied to boolean expressions.

| `a`     | `b`     | `a <-> b` |
| ------- | ------- | --------- |
| `true`  | `true`  | `true`    |
| `true`  | `false` | `false`   |
| `false` | `true`  | `false`   |
| `false` | `false` | `true`    |

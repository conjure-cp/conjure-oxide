<!-- maturity: draft
authors: Felix Leitner, Niklas Dewally, Hanaa Khan
created: 05-11-25
---- -->

<!-- TODO edit more -->

# Semantics of Rewriting Expressions with Side‐Effects

## Overview

---

When rewriting expressions, a rule may occasionally introduce new variables and constraints. For example, a rule which handles the `Min` expression may introduce a new variable `aux` which represents the minimum value itself, and introduce constraints `aux <= x` for each `x` in the original `Min` expression. Conjure and Conjure Oxide provide similar methods of achieving these "side-effects".

## Reductions

---

In Conjure Oxide, reduction rules return `Reduction` values. These contain a new expression, a new (possibly empty) set of top-level constraints, and a new (possibly empty) symbol table. The two latter values are brought up to the top of the AST during rewriting and joined to the model. These reductions, or "local sub-models" therefore can define new requirements which must hold across the model for the change that is made to a specific node of the AST to be valid.

The example with `Min` given in the overview is one case where plain `Reduction`s are used, as a new variable with a new domain is introduced, along with constraints which must hold for that variable.

## Bubbles

---

Reductions are not enough in all cases. For example, when handling a possibly undefined value, changes must be made in the context of the current expression. Simply introducing new top-level constraints may lead to incorrect reductions in these cases. The `Bubble` expression and associated rules take care of this, bringing new constraints up the tree gradually and expanding in the correct context.

Essentially, `Bubble` expressions are a way of attaching a constraint to an expression to say "X is only valid if Y holds". One example of this is how Conjure Oxide handles division. Since the division expression may possibly be undefined, a constraint must be attached to it which asserts the divisor is != 0.

An example of division handling is shown below. Bubbles are shown as `{X @ Y}`, where X is the expression that Y is attached to.
```
!(a = b / c)                 Original expression

!(a = {(b / c) @ c != 0})    b/c is possibly undefined, so introduce a bubble saying the divisor is != 0

!({(a = b / c) @ c != 0})    "bubble up" the expression, since b / c is not a bool-type expression

!(a = b / c /\ c != 0)       Now that X is a bool-type expression, we can simply expand the bubble into a conjunction
```

Why not just use `Reduction`s to assert at the top-level of the model that `c != 0`? In the context of undefinedness handling, the final reduction is dependent on the context it occurs in. In the above example, if we continue by simplifying (apply DeMorgan's), we can see that it becomes `a != b / c \/ c = 0`. Thus, c = 0 is a valid assignment for this example to be true, and setting `c != 0` on the top-level would be incorrect.

In Conjure Oxide, Bubbles are often combined with the power of `Reduction`s to provide support for solvers like Minion.
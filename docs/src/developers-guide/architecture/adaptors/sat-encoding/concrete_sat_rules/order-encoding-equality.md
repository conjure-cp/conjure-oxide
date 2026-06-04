# Order Encoding Equality and Inequality

## Overview

```text
SATInt(a) = SATInt(b) ~> Bool
SATInt(a) != SATInt(b) ~> Bool
```

## Rule Method
- Ensure both operands are in order encoding and have the same number of bits.
- Form a boolean expression by iterating over both bitvectors and asserting that corresponding bits are equivalent.
- For inequality, calculate the equality and negate the result.

### Comparison Logic
For two bitvectors $A$ and $B$ representing the order encoding of two integers:

$$
\begin{align}
A = [a_1, a_2, \dots, a_{n}],\\ 
B = [b_1, b_2, \dots, b_{n}]
\end{align}
$$

We can use the following expressions to encode equality/inequality:

**Equality**

$$
\begin{align}
(A = B) \quad \equiv \quad \boxed{(a_1 \Leftrightarrow b_1) \land (a_2 \Leftrightarrow b_2) \land \dots \land (a_n \Leftrightarrow b_n)} \\\\
\end{align}
$$

**Inequality**

$$
\begin{align}
(A \neq B) \quad \equiv \quad \boxed{\neg(A = B)}
\end{align}
$$


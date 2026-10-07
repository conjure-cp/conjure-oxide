# Log Encoding Equality and Inequality

## Overview

```text
SATInt(a) = SATInt(b) ~> Bool
SATInt(a) != SATInt(b) ~> Bool
```

## Rule Method
- Convert both operands to a canonical form by making both operands the same bit-wdith (using zero padding)
- Form a boolean expression by iterating over both bitvectors

### Comparison Logic
For two bitvectors that have been made the same length:

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
(A \neq B) \quad \equiv \quad \boxed{(a_1 \oplus b_1) \lor (a_2 \oplus b_2) \lor \dots \lor (a_n \oplus b_n)}
\end{align}
$$

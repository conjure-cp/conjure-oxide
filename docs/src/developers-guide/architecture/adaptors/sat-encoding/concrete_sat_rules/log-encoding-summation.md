# Log Encoding Summation

## Overview

```text
Sum(SATInt(a), SATInt(b), ...) ~> SATInt(c)
```

## Rule Method
- Determine the output range
- Pad all operands to match the output bitwidth to prevent overflow
- Split the expression into a series of 2-nary summations
- Use an binary adder circuit to perform each summation

### 2-nary Summation Logic
For two bitvectors that have been made the same length:

$$
\begin{align}
A = [a_1, a_2, \dots, a_{n}],\\ 
B = [b_1, b_2, \dots, b_{n}]
\end{align}
$$

Specifying the output

$$
S = A + B = [s_1, s_2, \dots, s_{n}]
$$

**Carry**

$$
\begin{align}
c_1 &= a_1 \land b_1 \\
c_2 &= (a_2 \land b_2) \lor ((a_2 \oplus b_2) \land c_1) \\
\vdots \\
c_{n-1} &= (a_{n-1} \land b_{n-1}) \lor ((a_{n-1} \oplus b_{n-1}) \land c_{n-2})
\end{align}
$$

**Sum**

$$
\begin{align}
s_1 &\equiv \boxed{a_1 \oplus b_1} \\
s_2 &\equiv \boxed{a_2 \oplus b_2 \oplus c_1} \\
\vdots \\
s_{n} &\equiv \boxed{a_{n} \oplus b_{n} \oplus c_{n-1}}
\end{align}
$$

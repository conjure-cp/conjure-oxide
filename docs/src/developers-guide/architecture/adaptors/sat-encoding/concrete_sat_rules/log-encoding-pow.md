# Log Encoding Multiplication

## Overview

```text
SafePow(SATInt(a), SATInt(b)) ~> SATInt(c)
```

`SafePow` defines the exponential $A^B$ operator under the condition that ($A \neq 0$ or $B \neq 0$) and $B \geq 0$. The implementation of this operation uses exponentiation by squaring:

To account for negatives, the rule works on the absolute value of $A$, $A^{+} = \left|A\right|$, and reapplies its sign at the end.

### Derivation

The core idea of this approach is to think of B with the following binary interpretation:

$$B = 2^0 b_0 + 2^1 b_1 + 2^2 b_2 + \dots + 2^{n-1} b_{n-1}, \qquad b_i \in \\{0,1\\}$$

So, to calculate $A^B$ we can use the following derivation:

$$
\begin{align}
A^B &= A^{2^0 b_0 + 2^1 b_1 + 2^2 b_2 + \dots + 2^{n-1} b_{n-1}} \\
&= A^{2^0 b_0} \cdot A^{2^1 b_1} \cdot A^{2^2 b_2} \dotsm A^{2^{n-1} b_{n-1}} \\
&= \prod_{i=0}^{n-1} A^{2^i b_i} \\
&= \prod_{i=0}^{n-1} \begin{cases} 
  A^{2^i}, & \text{if } b_i = 1 \\
  1,  & \text{if } b_i = 0 
\end{cases} \\
&= \prod_{i=0}^{n-1} \begin{cases} 
  S_i, & \text{if } b_i = 1 \\
  1,  & \text{if } b_i = 0 
\end{cases}
\end{align}
$$

$$
\begin{align}
S_0 &= A \\
S_i &= S_{i-1}^2 \qquad i=1,\ldots, n-1 \\
&\implies S_i = A^{2^i}
\end{align}
$$

Essentially, we can encode exponentials by decomposing the exponent $B$ into its binary representation and accumulating a running product over only those squaring steps $S_i$ where the corresponding bit $b_i = 1$.

### Algorithm
Maintain two quantities at each step $i = 0, 1,\ldots, n-1$:
- Squaring chain: $S_i = A^{2^i}$, updates as $S_i = S_{i-1}^2$
- Running product: $P_i$, the accumulated result so far

$$
P_0 = 1, \qquad P_i = P_{i-1} \cdot \begin{cases} 
  S_{i-1}, & \text{if } b_{i-1} = 1 \\
  1,  & \text{if } b_{i-1} = 0 
\end{cases}
$$

After all $n$ bits have been processed, $P_n = A^B$.

### Sign Handling
Since the algorithm operates on $A^+ = \left|A\right|$, the final result is:

$$
C = \begin{cases} 
  (A^+)^B, & \text{if } A \geq 0 \text{ or } B \text{ is even} \\
  -(A^+)^B,  & \text{if } A < 0 \text{ and } B \text{ is odd}
\end{cases}
$$

The parity of $B$ is available for free from its lowest bit: $B \text{ is odd} \iff b_0 = 1$.

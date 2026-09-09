## Encodings
This section outline the logic behind all of the SAT encoding rules

### Boolean
Boolean expressions are encoded via Tseytin transformations. The idea is that for a given boolean expression, an auxillary variable is created for each subexpression which is then substituted back into the main expression.

#### Example
Takes the following expression $\phi$:

```math
\phi := ((p\lor q) \land r) \rightarrow (\neg s)
```

Only subexpressions of literals can be encoded so we must look at the inner-most subexpressions, $((p\lor q)$ and $(\neg s))$, first:

$$
\begin{align}
x_1 \iff (p\lor q) \\
x_2 \iff (\neg s) \\
\end{align}
$$

$$
(((p\lor q) \land r) \rightarrow (\neg s)) \leadsto ((x_1 \land r) \rightarrow x_2)
$$

No we can transform $(x_1 \land r)$:

$$
x_3 \iff (x_1 \land r)
$$

$$
((x_1 \land r) \rightarrow x_2) \leadsto (x_3 \rightarrow x_2)
$$

And finally $(x_3 \rightarrow x_2)$:

$$
x_4 \iff (x_3 \rightarrow x_2)
$$

$$
(x_3 \rightarrow x_2) \leadsto x_4
$$

So we have:

$$
\begin{align}
x_1 \iff (p\lor q) \\
x_2 \iff (\neg s) \\
x_3 \iff (x_1 \land r) \\
x_4 \iff (x_3 \rightarrow x_2) \\
\phi \iff x_4
\end{align}
$$

And now to find solutions to $\phi$ we need to solve for $p,q,s, x_1, x_2, x_3, x_4$ under these constraints and the under additional constraint that $x_4$ is true.

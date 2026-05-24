## SAT Booleans

The most basic form of a constraint is a boolean expression. SAT solvers work with boolean variables, but require input in Conjunctive Normal Form (which will be referred to as CNF). This means that any boolean expression must be converted to a series of clauses composed entirely of atoms (true/false values or variables). Here's an example:

Original Expression:

$$((A \land B) \to (C \lor \lnot D)) \land E$$

In CNF:

$$(\lnot A \lor \lnot B \lor C \lor \lnot D) \land (E)$$

The simplest approach to this conversion is by repeatedly applying rules (De Morgan's, double negation, distributivity, etc) until the expression is in CNF. This works well for smaller expressions but slows down massively when dealing with larger expressions (like the once generated when dealing with integers). To solve this, we use Tseytin transformations (see <https://en.wikipedia.org/wiki/Tseytin_transformation>).

Boolean expressions are encoded via Tseytin transformations. 

### Tseytin Transfomations

These transformations allow us to convert an operation straight into CNF by introducing new auxiliary variables. This method makes rule applications significantly faster but with the downside of producing longer CNF with more variables to solve for. Modern SAT solvers, however, are very efficient and this extra performance cost is negligible compared to the time saved in conversion.

The idea is that for a given boolean expression, an auxillary variable is created for each subexpression which is then substituted back into the main expression. 

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

## Types of SAT Encodings

Conjure-oxide represents three SAT encoding styles in the AST. The three types are:

- Direct Encodings  
- Logarithmic Encodings  
- Order Encodings  

Each type of encoding has pros and cons, and a different one may be selected based on the type of constraint problem.

### SATInt Expression

All encodings are represented by the `SATInt` expression. This is represented by a 4-tuple variant with the following shape:

- `Metadata`: standard AST metadata (source location, typing info, etc.).
- `SATIntEncoding`: an enum selecting the encoding (`Direct`, `Log`, `Order`).
- `bits`: a (possibly nested) list/matrix of boolean expressions (literals, variables or composed expressions) encoded as `Moo<Expression>`.
- `range`: the inclusive (min, max) bounds for the integer, expressed as a pair of `i32`.

### Logarithmic Encoding

The base principle is quite simple: encode an integer as a bitvector. This allows us to represent integers as a series of boolean constraints -- one for each bit.

For example, the integer `6` can be represented in binary as `0110`. We can then represent this as `P = 0 ∨ 1, Q = 0 ∨ 1, R = 0 ∨ 1, S = 0 ∨ 1`. The connection that is missing, however, is that this isn't actually a representation of a constraint problem, but of a solution to a problem.  

### Direct Encoding

Direct encodings are the most straightforward type of encoding - it involves creating a vector of boolean variables, one corresponding to each member of the domain. Only one of these variables can be true at a time, and it is the one corresponding to the value of the integer.

### Order Encoding

Order follows the same principle as direct encoding, but instead of each boolean variable 'specifying' a value in the way that direct encodings do, each bit specifies whether the integer corresponding to it is less than or equal to the integer variable's value.

### Why have multiple types of encoding?

Only logarithmic encodings are currently implemented in conjure-oxide. We're planning to include other encodings such as direct and order encodings. This is motivated by their potential advantages over the log encoding in some cases.

Direct encodings should perform well for equality-heavy constraints but may scale poorly with larger domains or inequalities. Logarithmic encodings are expected to handle inequalities more efficiently. Order encodings are often viewed as a compromise, potentially balancing these trade-offs.

### Performance Comparison

The choice of encoding significantly impacts performance depending on the constraint types used.

#### Direct Encoding: Better for Equality and Disequality

Direct encoding excels in models dominated by `=` and `!=` constraints, such as graph coloring. It enables immediate unit propagation in the SAT solver, pruning values faster than bitwise reasoning.

**Example: Graph Coloring**

```essence
find c1, c2, c3, c4, c5 : int(1..3)
such that
    c1 != c2, c1 != c3, c2 != c3, c2 != c4,
    c3 != c4, c3 != c5, c4 != c5
```

- **Direct:** `~0.55s` Rewriting / `0.001s` Solving
- **Log:** `~1.61s` Rewriting / `0.002s` Solving

#### Logarithmic Encoding: Better for Inequalities

Logarithmic encoding is superior for arithmetic and inequalities (`<`, `>`, `<=`, `>=`) over large or sparse domains. Binary bit-vectors result in a more compact representation and reduced overhead.

**Example: Sparse Domain with Inequalities**

```essence
find x : int(1, 10, 20, 30, 40, 50)
such that x > 10
```

- **Direct:** `~1.24s` Rewriting / `0.005s` Solving
- **Log:** `~0.07s` Rewriting / `0.000s` Solving

#### Order Encoding: Middle ground for range/inequality-heavy models

Order encoding often sits between `Direct` and `Log` encodings in terms of performance and representation size. `Order` encodings are a pragmatic choice for models with many range/threshold constraints over moderate domains: they can outperform `Log` encodings on some inequality-heavy problems while avoiding the quadratic blow-up of very large `Direct` encodings.

TODO: Add benchmark here.

### Current Encoding Support Status

Currently, logarithmic encoding is mostly implemented; Direct and Order encodings have partial support in the SAT rules.

As of 27 April 2026 the Logarithmic encoding is the most feature-complete and is used for the bulk of SAT arithmetic and comparison rules. The Direct and Order encodings are partially implemented: both support core comparison functionality, while Direct has additional support for some arithmetic helpers and Order provides a foundation for inequalities. Several non-core features and edge-case helpers remain in development or unimplemented across the three encodings.

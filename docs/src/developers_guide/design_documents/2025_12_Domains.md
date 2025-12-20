<!-- maturity: draft
authors: Georgii Skorokhod
created: 20-11-2025
---- -->



<!-- steel yourselves, for this one is heavy in set notation... -->



<!-- TODOs -->



# State of Domains in Conjure-Oxide

## Background

### Definitions

In its basic form, a constraints problem $\text{CSP}(V, D, C)$ consists of:

- A set of **decision variables**, $V = \{x_1, ..., x_n\}$
- A set of **variable domains**, $D = \{D_1, ..., D_n\}$.
  The domain $D_i$ is a finite set of possible values that the variable $x_i$ may take.
- A set of **constraints** $C = \{c(x_1, ..., x_{k_1}),\, c(x_1, ..., x_{k_2}), \, ...\}$ upon the variables $x_1 ... x_k$.
  - Each constraint $c_i(x_1, ..., x_{k_i})$ restricts the values that variables $x_1..x_{k_i}$ 
    are allowed to take. Formally, it defines the set of **allowed tuples** $\{\langle d_1, ..., d_{k_i} \rangle\}$, 
    where $\forall i \, . \,d_i \in D_i$.

To define a solution to a CSP, let us first define some terms:

- An **assignment** of variables is a mapping of 0 or more variables to values 
  from their domains: $\{x_i \rightarrow (d_i \in D_i)\}$.
- A **complete assignment** is an assignment of all $n$ variables.

Then:

- A solution to $\text{CSP}(V, D, C)$ is a complete assignment of all variables in $V$ which satisfies all constraints in $C$.

  

### What is a Domain?

As defined above, the domain of a decision variable is the finite, ordered set of values that the variable is allowed to take. This is given as part of the problem specification. For example:

```
find x: int(1..5)
find y: set of int(2..4)
find z: matrix indexed by [int(1..2)] of (set of int(1..3))
```

Here:

- $x$ has domain $\{1, 2, ...,5\}$

- $y$ has domain $\{\emptyset, \{2\}, ...,\{4\}, \{2, 3\}, ..., \{2, 3, 4\}\}$

- $z$'s domain is the set of all matrices of size 2 whose elements are sets of integers 1..3

  

Note that in Essence a matrix domain also defines how the elements of a matrix are indexed.
The following:

- `x: matrix indexed by [int(0..2)] of int(1..5)`
- `y: matrix indexed by [int(3, 5, 42)] of int(1..5)`

Are both one-dimensional arrays of 3 elements, but the former's elements are accessed as:
```
-- x = [1, 2, 3]
x[0] = 1
x[1] = 2
x[2] = 3
```

While the latter's elements are accessed as:

```
-- y = [1, 2, 3]
y[3] = 1
y[5] = 2
y[42] = 3
```



### What Can Have a Domain?

Apart from decision variables, the following terms in Essence can have a domain:

- **Parameter declarations** (`given`s):
  Unlike decision variables, their domains can be (and, indeed, normally are) *infinite*. This is because, when instantiating the problem, the user supplies concrete values for each parameter, and thereon they become equivalent to constant `letting`s.

- **Expressions**:
  If the domains of all leaf sub-expressions (variables or constants) is known, the domain of an `Expression` containing them can usually also be inferred. For example:

  ```
  x: int(1..3, 5)
  b: bool
  
  toInt(b)     -- int(0,1)
  (x + 1) * 2  -- int(4, 6, 8, 12)
  ```

  There are some cases where this is not possible, and others where inferring a domain may be theoretically possible but too computationally expensive.

- **Literals**:

  Every literal $c$ can be said to belong to a single-value domain $\{c\}$ consisting only of itself.
  For example: 

  - `1 : int(1)`
  - `True: bool` 
  - `{1, 2, 3}: set (size 3) of int(1..3)`

- **Constant declarations** (`letting`s):
  Lettings contain an expression, so $dom(|| \verb |letting x be <expr>| ||) \equiv dom(|| \verb |expr| ||)$.



### List of Domains in Essence

### Domains vs Types

### Where Domains are Used

### Ground and Unresolved Domains



## The `Domain` enum

### `GroundDomain`

### `UnresolvedDomain`



## Constructing Domains



## Domain Operations 

### Size Bound

### Enumerating a Domain

### Union

### Intersection



## Future Work

### Evaluating Expressions Inside Domains

### Enumerating Abstract Domains

### Implementing `HasDomain` for AST Types

### Decision Variables Inside Domains

### Domain-Level Representations








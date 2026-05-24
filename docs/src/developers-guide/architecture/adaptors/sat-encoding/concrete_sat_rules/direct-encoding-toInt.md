# Direct Encoding toInt

## Overview

```text
toInt(bool) ~> SATInt
```

## Method
- Given a Boolean condition P, we want to set `r0` true when the condition does not hold: $r_0 \leftrightarrow ¬P$; and set `r1` true when condition P does hold: $r_1 \leftrightarrow P$.
- Each of these biconditionals are encoded as two clauses: one for each direction of the biconditional.
- We return a new CNF reduction with the vector `[r_0, r_1]` encoded as a direct SATInt.

## Example
Consider the following problem -

```essence
find x, y : int(0..3)
such that x = toInt(y=2)
```

The solutions produced are:
| x | y |
| :-- | :-- |
| 0 | 0 |
| 0 | 1 |
| 1 | 2 |
| 0 | 3 |

`x=1` only when the condition `y=2` holds, and `x=0` otherwise.
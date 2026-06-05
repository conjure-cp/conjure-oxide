# Order Encoding toInt

## Overview

```text
toInt(bool) ~> SATInt
```

## Method
- This rule follows the same method as seen in the direct encoding version.
- However, because are using order encoding uses thresholds (where $r_i$ means $value \geq i$), we can say that for $r_0$, $value \geq 0$ will always be true, thus can set $r_0$ to true directly.
- We encode the biconditional $r_1 \leftrightarrow P$ as in the direct encoding version.

An example of this rule can also be found in the direct encoding page on this rule.
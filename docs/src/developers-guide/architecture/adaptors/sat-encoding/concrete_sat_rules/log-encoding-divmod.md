# Log Encoding Division & Modulo

## Overview

```text
SafeDiv(SATInt(a), SATInt(b)) ~> SATInt(c)
SafeMod(SATInt(a), SATInt(b)) ~> SATInt(c)
```

Division and modulo are strongly linked, for log encodings the Restoring Division Algorithm (https://en.wikipedia.org/wiki/Division_algorithm#Restoring_division) is used, performing this algorithm gives you the quotient and remainder which can be used for division and modulo respectively. One very important thing to bear in mind is that there are multiple competing standards for how negatives should be handled. So when implementing these rules, it is important to ensure that you implement the correct standard for conjure:

**Summary**

$$
\begin{align}
3 &/\\, 2 &= 1 \\
-3 &/\\, 2 &= -2 \\
3 &/\\, -2 &= -2 \\
-3 &/\\, -2 &= 1 \\
\end{align}
$$

$$
\begin{align}
3 \\,&\\%\\, 2 &= 1 \\
3 \\,&\\%\\, -2 &= -1 \\
-3 \\,&\\%\\, 2 &= 1\\
-3 \\,&\\%\\, -2 &= -1
\end{align}
$$

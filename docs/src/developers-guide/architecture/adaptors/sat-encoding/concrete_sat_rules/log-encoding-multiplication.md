# Log Encoding Multiplication

## Overview

```text
Poduct(SATInt(a), SATInt(b), ...) ~> SATInt(c)
```

## Rule Method
- Determine the output range
- Pad all operands to match the output bitwidth to prevent overflow
- Split the expression into a series of 2-nary products
- Use shift-add binary multiplication for each product

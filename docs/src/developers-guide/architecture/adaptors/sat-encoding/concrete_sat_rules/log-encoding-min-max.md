# Log Encoding Min/max

## Overview

```text
Min(SATInt(a), SATInt(b), ...) ~> SATInt(c)
Max(SATInt(a), SATInt(b), ...) ~> SATInt(c)
```

## Rule Method
- Iterate over every integer operand
- Update a cumulative min/max by with the existing log comparison functionality

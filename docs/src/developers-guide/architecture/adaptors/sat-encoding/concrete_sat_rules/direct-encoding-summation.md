# Direct encoding summation

A direct-encoded operand contributes one weighted indicator for each actual domain value. A sum combines these weighted views and uses the chosen PB provider, rather than enumerating every pair of possible summands into a new direct-encoded result.

When the sum needs an integer result for a nonlinear operation, a represented auxiliary and a PB equality connect that result to the weighted sum. See [integer summation](log-encoding-summation.md) for the shared lowering path.

# SAT Booleans

CNF is generated only in the adaptor. Assertion context is preserved: an asserted conjunction compiles its children directly, and a disjunction or implication can become a clause without an auxiliary output for every subexpression.

For example, asserting `((a AND b) -> (c OR NOT d)) AND e` produces the clauses `(NOT a OR NOT b OR c OR NOT d)` and `(e)`. Larger asserted formulas may need witnesses, but a witness used only positively needs only the direction from the witness to its formula.

Boolean expressions used as values, numeric operands or named definitions require full equivalence. A shared gate therefore records both directions, as in `x <-> (p OR q)`. Negation uses the opposite literal instead of a new gate. Constants, duplicates, complements and known aliases simplify inputs before clause generation through RustSAT's atomic encodings.

The cache distinguishes positive assertion witnesses from equivalent gates. If a later incremental batch needs a witness as a value, compilation adds the missing reverse implication before reusing it. This distinction preserves both truth values for reification while avoiding unnecessary clauses for assertions.

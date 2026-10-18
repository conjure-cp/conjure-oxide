# SAT encoding

Oxide records modelling and encoding decisions in a semantic SAT IR. The model contains Boolean expressions, numeric views and selected algorithms; it contains no CNF clauses or solver literals. The SAT adaptor compiles those decisions directly into a RustSAT `SatInstance`, using RustSAT and Pindakaas encoders where their public interfaces fit.

Representation rules first expose Boolean operands and actual-value integer views. The rule engine then resolves unpinned encoding choices through the configured heuristic. Command-line choices take precedence and their provenance is retained in the IR. See [integer representations](encoding-types.md), [Boolean compilation](booleans.md) and [the adaptor](sat-adaptor.md).

Uniform channelling chooses one representation for each type family across a model. Integer representation is independent of matrix layout. SAT integration coverage currently concentrates on this mode; broader channelling coverage remains future work.

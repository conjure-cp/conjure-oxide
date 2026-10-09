# Creating SAT transformation rules

SAT rules refine semantic expressions and record encoding decisions. They do not create CNF clauses or solver literals. Prefer an existing numeric view and a library-backed decision over a new circuit implementation.

1. Validate the target expression and operand types; decline until required representations are available.
2. Extract ordered operands or actual-value integer views using shared helpers. Preserve sparse domain values, matrix order and any guaranteed representation groups.
3. Construct the semantic result. Linear comparisons use integer-relation or PB decisions; table, element and allDifferent have dedicated decisions. Nonlinear rules can still build Boolean expressions for their circuits.
4. If a result needs a named Boolean or integer auxiliary, allocate it in the symbol table and record its definition. Do not assign a solver literal here.
5. Return `RuleEffect::sat` when adding SAT decisions, or `RuleEffect::pure` for a semantic decomposition. Propagate output domains and representation constraints separately.

Leave an encoding selection unresolved unless the rule itself fixes an algorithm. After rewriting, `rule_engine/encoding_selection.rs` applies explicit family pins or the configured heuristic and records the selection's provenance. The adaptor then validates and compiles the decisions directly through RustSAT and Pindakaas.

All integer representations share the `SAT` rule set. Representation-specific rules must decline mismatched operands; native weighted views let common linear rules handle several representations together. Two's-complement fallback remains useful for nonlinear operations, rather than being the default path for linear arithmetic.

Use the current registered rules as priority examples: representation materialisation must precede rules that inspect numeric views, compound decomposition must precede scalar-only lowering, and final assertion handling must preserve Boolean context. The former fixed 4000-level priority table no longer describes this pipeline.

Useful starting points are `backends/sat/pseudo_boolean.rs`, `backends/sat/table.rs` and `backends/sat/boolean.rs`. Representation-dependent refinement belongs under the corresponding type's representation-specific vertical rules. See [Boolean compilation](booleans.md) for assertion and full-equivalence requirements.

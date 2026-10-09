# Integer summation

Linear sums compile through actual-value weighted views rather than materialised ripple-adder results. Each operand contributes its constant and signed Boolean terms. A comparison of the sum becomes an integer-relation or pseudo-Boolean decision, and the selected RustSAT or Pindakaas provider creates CNF in the adaptor.

For example, `x + y <= k` combines the two views and subtracts their constant contributions from `k`. The adaptor folds constants, aggregates repeated literals and complements, and converts signed coefficients into positive weights over appropriately polarised literals. Guaranteed representation groups are retained for structured Pindakaas inputs where possible.

When a sum is itself an operand of a nonlinear operation, Oxide introduces an integer auxiliary in the selected representation and a PB equality relating its view to the sum. Multiplication, division, remainder and power can then use the existing nonlinear bit circuits. Addition internal to those circuits remains circuit logic; ordinary model-level summation does not pass through that path.

The relevant implementations are `backends/sat/pseudo_boolean.rs` and `backends/sat/linear.rs` in the rules crate, and `solver/adaptors/rustsat/decisions/pseudo_boolean.rs` in core.

# SAT integer representations

Oxide supports six integer representations. Every representation supplies an actual-value view consisting of a constant and signed weights over Boolean expressions. Representation constraints independently restrict the possible values; numeric relation and pseudo-Boolean encoders consume the view rather than the representation's bit layout.

| Representation | Meaning | Sparse domains |
| --- | --- | --- |
| Direct | One indicator for each domain value, with exactly one true | Only actual domain values have indicators |
| Order | A chain of thresholds between consecutive domain values | Threshold differences use the gaps between actual values |
| BinaryValue (`Log` in the AST) | Actual numeric value in binary, using two's complement for signed domains | Domain constraints exclude holes |
| BinaryOffset | Unsigned displacement from the domain minimum | Domain constraints exclude disallowed displacements |
| BinaryRank | Unsigned ordinal in the sorted domain | Each ordinal denotes an actual domain value |
| SignMagnitude | Magnitude bits and a sign bit | Domain constraints exclude holes and duplicate zero |

`SATInt` records metadata, a `SATIntEncoding`, Boolean representation operands and inclusive bounds. Rank additionally retains the canonical domain intervals in its encoding variant. These are semantic expressions, not solver literals.

Direct and Order yield weighted views over their indicators and thresholds. BinaryValue and BinaryOffset give affine weighted views over their bits. Dense BinaryRank is affine too; sparse BinaryRank requires decoding ordinals into values. Rank and offset therefore remain distinct even when their bit widths coincide.

For SignMagnitude, each magnitude bit contributes `2^i * bit - 2^(i+1) * (sign AND bit)`. Shared Boolean gates represent the conjunctions. This avoids converting linear relations through two's complement merely to compare or sum signed values.

Linear constraints use the selected PB provider across these representations. Nonlinear multiplication, division, remainder and power still use Oxide's bit circuits where the libraries expose no suitable public result-value encoder. There is no universal fastest representation: domain size, gaps and constraint structure all matter. Compact makes basic structural choices; explicit pins and portfolio heuristics allow alternatives.

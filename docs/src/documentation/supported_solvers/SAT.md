# What is SAT

SAT stands for the Boolean Satisfiability Problem. (TODO: add resources)

# SAT Adaptor

Translates Conjure-Oxide models into CNF format for SAT solvers.

## Components

### Translation Layer
- Integers -> bit vectors (`SATInt`)
- Constraints -> CNF clauses via Tseytin transformations

### Rewrite Rules (`crates/conjure-cp-rules/src/sat/`)
- `boolean.rs`: Logic operations (AND, OR, NOT) to CNF
- `integer_repr.rs`: Integer variables to bit-vectors
- `integer_operations.rs`: Integer comparisons and arithmetic on bits

### Solver Interface (`crates/conjure-cp-core/src/solver/adaptors/rustsat/`)
- Implements `SolverAdaptor` trait
- Uses Minisat via RustSAT
- Translates boolean solutions back to Essence variables

## Integer Encodings

Currently uses **logarithmic encoding** (binary): an 8-bit integer uses 8 boolean variables.

Planned encodings:
- **Direct encoding**: Each possible value gets one boolean variable (e.g., `x=5` → `x_5`)
- **Order encoding**: Each value gets a boolean indicating `x >= k`

### When Log Encoding Falls Short

**Example: Some problem**
```
Very ubiquitous problem where log would do worse than direct or order.
```

Ooga booga look how cool it is to have multiple encodings
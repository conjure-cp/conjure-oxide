# Diss ToDos

## Writing

- [ ] Abstract (250 words)
- [ ] Introduction
  - [ ] Project background (adapt from interim)
  - [ ] Objectives
- [ ] Definitions?
- [ ] Context survey
  - [ ] Theoretical background - Chris Jefferson diss
  - [ ] Conjure design / structure - Oz diss
  - [ ] Other modelling systems? (Minizinc etc)
  - [ ] Smth on type-driven development / encoding logical requirements as types (Idris?)
  - [ ] Smth on functors?
  - [ ] Cite Nik for uniplate
- [ ] Requirements spec (adapt from interim)
  - [ ] Functional
  - [ ] Non-functional
- [ ] SE process
- [ ] Ethics
- [ ] Design
  - [ ] Principle 1: representations at domain level
    (more elegant definitions, can cache / reuse work)
  - [ ] Principle 2: user controls the state structure used
    (examples of reprs that need different data structures, eg mta vs sets vs int encodings)
  - [ ] Principle 3: representation states as types, transitions between them as functors
  - [ ] Horizontal and vertical rules
  - [ ] Repr selection (pre-planned vs one level at a time)
- [ ] Implementation
  - [ ] GATs and family-of-types pattern in Rust
  - [ ] Typemap pattern
  - [ ] Functors
  - [ ] MTA repr: indexing optimisations etc
  - [ ] Representation macros
- [ ] Evaluation
  - [ ] Present + discuss example of multi-step representation (e.g record -> tuple -> atom)
  - [ ] Present + discuss nested representation (record of records / matrix in record / etc)
  - [ ] Present + discuss repr of constants and domain lettings
  - [ ] Present + discuss multiple representation + channeling example (set expl + occ)
  - [ ] Performance comparisons
  - [ ] Limitations
- [ ] Conclusion
- [ ] Further work





## Experiments

### Example Models

- [ ] Nested tuples
- [ ] Nested records
- [ ] Matrix indexed by abstract
- [ ] Set with 2 representations

### Perf Benchmarks

- Benchmark against:
  - [ ] Oxide main
  - [ ] Old conjure + Savile Row
  - [ ] Savile Row by itself
- Metrics:
  - [ ] Rewrite time
  - [ ] Solver time
  - [ ] Total wall clock time
  - [ ] Memory usage?

- [ ] BIBD
- [ ] Pythagorean Triples
- [ ] Large nested records / tuples





## Rules

### Set Explicit

- [ ] Vertical rule - `a in b`
- [ ] Vertical rule - equality
- [ ] Vertical rule - cardinality
- [ ] Selection rule



### Set Occurrence

- [ ] Vertical rule - `a in b`
- [ ] Vertical rule - equality
- [ ] Vertical rule - cardinality
- [ ] Vertical rule - intersection / union
- [ ] Channeling to explicit



### Records / Tuples

- [ ] General code for inequalities
- [ ] Vertical rule - `a > b`
- [ ] Vertical rule - `a >= b`
- [ ] Vertical rule - `a <= b`
- [ ] Vertical rule - `a < b`
- [ ] Vertical rule - lex constraints?



### Matrix To Atom

- [ ] Slicing for non-integer indices
- [ ] Better index remapping



### Domains

- [ ] Enumerating abstract domains


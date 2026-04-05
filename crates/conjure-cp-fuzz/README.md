# conjure-cp-fuzz

AFL-based fuzzing harness for Conjure Oxide.

Feeds mutated Essence model source text through the full pipeline
(parse → rewrite → solve) with AFL's coverage-guided instrumentation to
discover inputs that exercise new code paths — particularly in the
representation and rewriting systems.

## Prerequisites

```bash
cargo install cargo-afl
```

## Building

```bash
cargo afl build -p conjure-cp-fuzz --release
```

## Running

```bash
cargo afl fuzz \
    -i crates/conjure-cp-fuzz/seeds \
    -o crates/conjure-cp-fuzz/corpus \
    -x crates/conjure-cp-fuzz/essence.dict \
    target/release/conjure-fuzz-harness
```

## Seeds

Place hand-written `.essence` files in `seeds/`. These are the starting
inputs that AFL will mutate. Good seeds should cover the model shapes
you care about — matrices, sets, tuples, records, comprehensions,
quantifiers, etc. Aim for 20–30 small, diverse models.

## Corpus

AFL writes its discovered interesting inputs into `corpus/`. After a
fuzzing session you can harvest the queue:

```bash
ls corpus/default/queue/
```

Each file is a valid (or near-valid) Essence model that triggered new
coverage. You can then batch-run these through different backends for
differential comparison.

## Dictionary

`essence.dict` contains Essence tokens that AFL will preferentially use
during mutation, dramatically improving the rate of syntactically valid
inputs.


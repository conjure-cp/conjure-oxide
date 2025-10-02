## `conjure-cp-rules`

Rewrite rules for `conjure-oxide`.

### Usage

To use these rules with the [`conjure-oxide`
CLI](https://github.com/conjure-cp/conjure-oxide/crates/conjure-cp-cli), no
further installation is needed.

To use these rules with
[`conjure-cp`](https://github.com/conjure-cp/conjure-oxide/crates/conjure-cp),
first add `conjure-cp-rules` to your `Cargo.toml`:

```toml
[dependencies]
conjure-cp-rules = {git = "https://github.com/conjure-cp/conjure-oxide" }
```

Then add the following to any Rust file:

```rs
use conjure_cp_rules as _;
```

### Licence

This project is licenced under the [Mozilla Public Licence
2.0](https://www.mozilla.org/en-US/MPL/2.0/).

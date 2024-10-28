

The following compiler flags are required for Conjure-Oxide to work with
Nightly Rust:

```sh
export RUSTFLAGS="-Zlinker-features=-lld" 
export RUSTDOCFLAGS="-Zlinker-features=-lld" 
cargo build <...>
```

This is because of current incompatibilities with linkme and the new default
linker ([link](https://github.com/dtolnay/linkme/issues/94)).


## Documentation

API documentation can be found [here](https://conjure-cp.github.io/conjure-oxide/docs/).

## Contributing

See the [project wiki](https://github.com/conjure-cp/conjure-oxide/wiki)
<!-- vim: cc=80
-->

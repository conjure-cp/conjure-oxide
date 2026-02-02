<!-- maturity: draft
authors: Felix Leitner
created: 01-02-26
---- -->

# Integration Testing

<!-- TODO: expand this -->

Tests can be filtered with an optional argument to `cargo test`. 

```bash
cargo test integration_basic
```

The above command will run only the tests with the substring "integration_basic" in their name. Since tests are named after their relative path, this includes only the tests in the `integration/basic` directory.

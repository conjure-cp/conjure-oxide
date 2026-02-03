<!-- maturity: draft
authors: Felix Leitner
created: 01-02-26
---- -->

# Environment Variables

Environment variables can be used to configure various aspects of the tool for debugging and testing.

## Integration Testing Variables

Some variables can be used with `cargo test` to enable/disable various steps of the integration tests. See [Integration Testing](./integration_testing.md) for background on our integration testing framework.

These variables can be enabled per-command, rather than cluttering your shell environment after being set:

```bash
ACCEPT=true cargo test
```

Useful variables are included below. Those marked with "Test Config" can also be set in an individual test's `config.toml` using lowercase `snake_case`. If set, the environment variable will always overwride the configured value.

| Variable Name | Possible Values | Test Config | Effect |
| ------------- | --------------- | ----------- | ------ |
| ACCEPT        | `true` `false`  |  | Accepts all output as ground truth and overwrite "expected" files. This includes parsing, rule rewriting (trace and rewritten model), and solutions.
| VERBOSE       | `true` `false`  |  | Prints additional information for each testing stage, including the rewritten model and solutions.
| SAVE_INPUT | `true` `false` | | Saves the rewritten problem to an untracked file in the solver-specific format.
| ENABLE_MORPH_IMPL | TODO | ✓ | TODO
| ENABLE_NAIVE_IMPL | TODO | ✓ | TODO
| ENABLE_NATIVE_PARSER | TODO | ✓ | TODO
| APPLY_REWRITE_RULES | TODO | ✓ | TODO
| ENABLE_EXTRA_VALIDATION | TODO | ✓ | TODO
| SOLVE_WITH_MINION | TODO | ✓ | TODO
| SOLVE_WITH_SAT | TODO | ✓ | TODO
| SOLVE_WITH_SMT | TODO | ✓ | TODO
| SAT_ENCODING | TODO | ✓ | TODO
| COMPARE_SOLVER_SOLUTIONS | TODO | ✓ | TODO
| VALIDATE_RULE_TRACES | TODO | ✓ | TODO
| ENABLE_REWRITER_IMPL | TODO | ✓ | TODO

## Debugging Variables



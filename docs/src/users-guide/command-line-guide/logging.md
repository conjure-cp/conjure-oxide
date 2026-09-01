# Logging

**This document is a work in progress - for a full list of logging options,
see Conjure Oxide's `--help` output.**

## To `stderr`

The verbosity flags form a detail ladder:

+ With no flag, only warnings are logged.
+ `-q` / `--quiet` disables logs on `stderr`.
+ `-v` logs concise stage information such as parsing, rewriting, and running the solver.
+ `-vv` additionally logs successful rule applications.
+ `-vvv` additionally logs every rule attempt, including failures. This can be expensive and
  produce a very large amount of output.

The **`RUST_LOG`** environment variable remains available as an advanced override for selecting
levels or modules when no verbosity flag is supplied. Explicit `-v`/`-vv`/`-vvv` and `--quiet`
take precedence over it.

## To a file

`--log-file <PATH>` creates one general log file. Its format and detail are independent of the
terminal verbosity:

```sh
conjure-oxide solve model.essence \
  --log-file attempts.json \
  --log-format json \
  --log-detail attempts
```

`--log-format` accepts `text` (the default) or `json`. `--log-detail` accepts `stages` (the
default), `applications`, or `attempts`. The supplied filename is used exactly as written and is
truncated at the start of the run.

### Example: Logging Rule Applications

Different log levels provide different information about the rules applied to
the model:

+ `-vv` prints rules that were successfully applied to the model.

+ `-vvv` additionally prints every attempted rule and whether it succeeded.

To see TRACE logs in a pretty format (mainly useful for
debugging):

```sh
conjure-oxide solve -vvv <model>
```

Or, using cargo:

```sh
cargo run -- solve -vvv <model>
```

### Example: Tracing SAT Solver Rules

When working with the SAT solver, you can trace the complete transformation pipeline:

```bash
cargo run -- solve --solver sat -vvv my_problem.essence
```

This will show:

+ Integer-to-boolean conversions
+ Operation transformations
+ Tseytin transformations
+ All rules that were tried and applied

> Which integer encoding each variable gets -- `int_log`, `int_direct` or `int_order` -- is a
> representation choice made per declaration rather than part of the solver name. Use
> `--heuristic` to steer it: `-h i` prompts for each choice, and `-h c` (the default) takes the
> most compact.

For more detailed testing output (including JSON traces and rewritten models), run specific tests:

```bash
cargo nextest run -E 'test(<test_name>)'
```

This generates `.json` and `.txt` files containing rule traces, parsed Essence, solutions, and the rewritten model.

# Logging 

**This document is a work in progress - for a full list of logging options,
see Conjure Oxide's `--help` output.**

## To `stderr`

Using `--verbose`, and the `RUST_LOG` environment variable, you can control the
contents, and formatting of, Conjure Oxide's `stderr` output:

+ **`--verbose`** changes the formatting of the logs for improved readability,
  including printing source locations and line numbers for log events. It also
  enables the printing of the log levels `INFO` and above.

+ The **`RUST_LOG`** environment variable can be used to customise the
  log levels that are printed depending on the module . For more
  information, see:
  <https://docs.rs/env_logger/latest/env_logger/#enabling-logging>.

### Example: Logging Rule Applications

Different log levels provide different information about the rules applied to
the model:

+ `INFO` provides information on the rules that were applied to the model. 

+ `TRACE` additionally prints the rules that were attempted and why they were
  not applicable. 


To see TRACE logs in a pretty format (mainly useful for
debugging):

```sh
$ RUST_LOG=trace conjure-oxide solve --verbose <model>
```

Or, using cargo:

```sh
$ RUST_LOG=trace cargo run -- solve --verbose <model>
```



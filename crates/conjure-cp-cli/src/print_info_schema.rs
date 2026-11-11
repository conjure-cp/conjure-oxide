//! conjure-oxide print-json-schema subcommand

use conjure_cp_cli::stats::TestRunStats;
use schemars::schema_for;

/// Prints the schema for JSON statistics written by `--stats`.
pub fn run_print_info_schema_command() -> anyhow::Result<()> {
    let schema = schema_for!(TestRunStats);
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
    Ok(())
}

//! conjure-oxide print-info-schema subcommand

use conjure_cp::context::Context;
use schemars::schema_for;

/// Prints the schema for the JSON info file.
pub fn run_print_info_schema_command() -> anyhow::Result<()> {
    let schema = schema_for!(Context);
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
    Ok(())
}

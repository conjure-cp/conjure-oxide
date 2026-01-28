use std::{path::PathBuf, sync::Arc};

use anyhow::anyhow;
use clap::ValueHint;

use conjure_cp_cli::utils::testing::serialize_model;

use crate::cli::{GlobalArgs, LOGGING_HELP_HEADING};
use crate::solve::{init_context, parse};

#[derive(Clone, Debug, clap::Args)]
pub struct Args {
    /// The input Essence file
    #[arg(value_name = "INPUT_ESSENCE", value_hint = ValueHint::FilePath)]
    pub input_file: PathBuf,

    // The format you would like to print out (e.g. ast-json)
    #[arg(long, help_heading=LOGGING_HELP_HEADING)]
    pub output_format: String,
}

pub fn run_pretty_command(global_args: GlobalArgs, pretty_args: Args) -> anyhow::Result<()> {
    // Preamble
    let input_file = pretty_args.input_file.clone();
    let context = init_context(&global_args, input_file)?;
    let model = parse(&global_args, Arc::clone(&context))?;

    // Running the correct method to acquire pretty string
    let output = match pretty_args.output_format.as_str() {
        "ast-json" => serialize_model(&model),
        // "add_new_flag" => method(),
        _ => {
            return Err(anyhow!(
                "Unknown output format {}; supports [ast-json]",
                &pretty_args.output_format
            ));
        }
    };

    let output = output.map_err(|err| anyhow!("Could not pretty print: {err}"))?;
    println!("{output}");
    Ok(())
}

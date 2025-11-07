use std::{
    path::PathBuf,
    sync::{Arc},
};

use anyhow::{anyhow, Result};
use clap::ValueHint;

use conjure_cp_cli::utils::testing::{serialise_model};

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

pub fn run_pretty_command(global_args: GlobalArgs, pretty_args: Args) -> anyhow::Result<(), > {
    // Preamble
    let input_file = pretty_args.input_file.clone();
    let context = init_context(&global_args, input_file)?;
    let model = parse(&global_args, Arc::clone(&context))?;

    // Defining the variable to store the output;
    let output: Result<String, serde_json::Error>; 
    
    // Running the correct method to acquire pretty string
    match pretty_args.output_format.as_str() {
        "ast-json" => output = serialise_model(&model),
        _ => panic!() // TODO: Sort that mess,
    }

    if output.is_ok() {
        println!("{}", output.unwrap());
        Ok(())
    } else {
        Err(anyhow!("Could not pretty print"))
    }
}

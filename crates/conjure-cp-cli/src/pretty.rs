use std::path::PathBuf;

use anyhow::anyhow;
use clap::ValueHint;

use conjure_cp_cli::utils::testing::{serialize_domains, serialize_model};

use crate::cli::{GlobalArgs, LOGGING_HELP_HEADING};
use conjure_cp::pipeline::Pipeline;

#[derive(Clone, Debug, clap::Args)]
pub struct Args {
    /// The input Essence file
    #[arg(value_name = "INPUT_ESSENCE", value_hint = ValueHint::FilePath)]
    pub input_file: PathBuf,

    /// Output format: essence, ast-json, or expression-domains.
    #[arg(long, default_value = "essence", help_heading=LOGGING_HELP_HEADING)]
    pub output_format: String,
}

pub fn run_pretty_command(global_args: GlobalArgs, pretty_args: Args) -> anyhow::Result<()> {
    let pipeline = Pipeline::new(global_args.library_config())?;
    pipeline.context().write().unwrap().essence_file_name =
        Some(pretty_args.input_file.to_string_lossy().into_owned());
    let model = pipeline.parse_file(&pretty_args.input_file)?;

    // Running the correct method to acquire pretty string
    let output = match pretty_args.output_format.as_str() {
        "essence" => Ok(model.to_string()),
        "ast-json" => serialize_model(&model),
        "expression-domains" => serialize_domains(&model),
        _ => {
            return Err(anyhow!(
                "Unknown output format {}; supports [essence, ast-json, expression-domains]",
                pretty_args.output_format
            ));
        }
    };

    let output = output.map_err(|err| anyhow!("Could not pretty print: {err}"))?;
    print!("{output}");
    Ok(())
}

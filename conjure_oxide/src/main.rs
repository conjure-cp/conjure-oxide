// (niklasdewally): temporary, gut this if you want!

use anyhow::{anyhow, bail};
use std::path::PathBuf;

use anyhow::Result as AnyhowResult;
use clap::{arg, command, Parser};
use conjure_oxide::find_conjure::conjure_executable;
use conjure_oxide::parse::model_from_json;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(long, value_name = "SOLVER")]
    solver: Option<String>,

    #[arg(value_name = "INPUT_ESSENCE")]
    input_file: PathBuf,
}

pub fn main() -> AnyhowResult<()> {
    println!(
        "Rules: {:?}",
        conjure_rules::get_rules()
            .iter()
            .map(|r| r.name)
            .collect::<Vec<_>>()
    );

    let cli = Cli::parse();
    println!("Input file: {}", cli.input_file.display());
    let input_file: &str = cli.input_file.to_str().ok_or(anyhow!(
        "Given input_file could not be converted to a string"
    ))?;

    /******************************************************/
    /*        Parse essence to json using Conjure         */
    /******************************************************/

    conjure_executable()
        .map_err(|e| anyhow!("Could not find correct conjure executable: {}", e))?;
    let mut cmd = std::process::Command::new("conjure");
    let output = cmd
        .arg("pretty")
        .arg("--output-format=astjson")
        .arg(input_file)
        .output()?;

    let conjure_stderr = String::from_utf8(output.stderr)?;
    if !conjure_stderr.is_empty() {
        bail!(conjure_stderr);
    }

    let astjson = String::from_utf8(output.stdout)?;

    let model = model_from_json(&astjson)?;
    println!("{:?}", model);

    Ok(())
}

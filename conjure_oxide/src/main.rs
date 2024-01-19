// (niklasdewally): temporary, gut this if you want!

use anyhow::{anyhow, bail};
use std::path::PathBuf;

use anyhow::Result as AnyhowResult;
use clap::{arg, command, Parser};
use conjure_macros::rule;
use conjure_oxide::ast::*;
use conjure_oxide::find_conjure::conjure_executable;
use conjure_oxide::parse::model_from_json;
use conjure_oxide::rule::*;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(long, value_name = "SOLVER")]
    solver: Option<String>,

    #[arg(value_name = "INPUT_ESSENCE")]
    input_file: PathBuf,
}

pub fn main() -> AnyhowResult<()> {
    #[rule(Horizontal)]
    fn example_rule(_expr: Expression) -> RuleApplicationResult {
        Err(RuleApplicationError::RuleNotApplicable)
    }

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

    // for rule in get_rules_by_kind() {
    //     println!("Applying rule {:?}", rule);
    // }

    Ok(())
}

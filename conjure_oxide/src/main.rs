// (niklasdewally): temporary, gut this if you want!

use anyhow::Result as AnyhowResult;
use anyhow::{anyhow, bail};
use clap::{arg, command, Parser};
use conjure_oxide::find_conjure::conjure_executable;
use conjure_oxide::parse::model_from_json;
use conjure_oxide::rule_engine::resolve_rules::{
    get_rule_priorities, get_rules_vec, resolve_rule_sets,
};
use conjure_oxide::rule_engine::rewrite::rewrite_model;
use conjure_oxide::utils::conjure::{get_minion_solutions, minion_solutions_to_json};
use std::path::PathBuf;
use std::process::exit;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(long, value_name = "SOLVER")]
    solver: Option<String>,

    #[arg(
        value_name = "INPUT_ESSENCE",
        default_value = "./conjure_oxide/tests/integration/xyz/input.essence"
    )]
    input_file: PathBuf,
}

pub fn main() -> AnyhowResult<()> {
    let rule_sets = match resolve_rule_sets(vec!["Minion", "Constant"]) {
        Ok(rs) => rs,
        Err(e) => {
            eprintln!("Error resolving rule sets: {}", e);
            exit(1);
        }
    };

    println!("Rule sets:");
    print!("{{ ");
    rule_sets.iter().for_each(|rule_set| {
        print!("{}, ", rule_set.name);
    });
    print!("}}\n\n");

    let rule_priorities = get_rule_priorities(&rule_sets)?;
    let rules_vec = get_rules_vec(&rule_priorities);

    println!("Rules and priorities:");
    rules_vec.iter().for_each(|rule| {
        println!("{}: {}", rule.name, rule_priorities.get(rule).unwrap_or(&0));
    });
    println!();

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

    let mut model = model_from_json(&astjson)?;

    println!("Initial model:");
    println!("{:#?}", model);

    println!("Rewriting model...");
    model = rewrite_model(&model, &rule_sets)?;

    println!("\nRewritten model:");
    println!("{:#?}", model);

    let solutions = get_minion_solutions(model)?;
    println!("Solutions: {:#}", minion_solutions_to_json(&solutions));

    Ok(())
}

// (niklasdewally): temporary, gut this if you want!

use anyhow::{anyhow, bail};
use std::collections::HashMap;
use std::ops::Deref;
use std::path::PathBuf;
use std::process::exit;
use std::sync::Mutex;

use anyhow::Result as AnyhowResult;
use clap::{arg, command, Parser};
use conjure_oxide::find_conjure::conjure_executable;
use conjure_oxide::parse::model_from_json;
use conjure_oxide::rule_engine::resolve_rules::{
    get_rule_priorities, get_rules_vec, resolve_rule_sets,
};
use conjure_oxide::rule_engine::rewrite::rewrite_model;
use conjure_oxide::solvers::FromConjureModel;

use minion_rs::ast::{Constant, Model as MinionModel, VarName};
use minion_rs::run_minion;

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

static ALL_SOLUTIONS: Mutex<Vec<HashMap<VarName, Constant>>> = Mutex::new(vec![]);

fn callback(solutions: HashMap<VarName, Constant>) -> bool {
    let mut guard = ALL_SOLUTIONS.lock().unwrap();
    guard.push(solutions);
    true
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
    println!("{:?}", model);

    println!("Rewriting model...");
    model = rewrite_model(&model, &rule_sets)?;

    println!("\nRewritten model:");
    println!("{:?}", model);

    println!("Building Minion model...");
    let minion_model = MinionModel::from_conjure(model)?;

    println!("Running Minion...");
    let res = run_minion(minion_model, callback);
    res.expect("Error occurred");

    // Get solutions
    let guard = ALL_SOLUTIONS.lock().unwrap();
    guard.deref().iter().for_each(|solution_set| {
        println!("\nSolution set:");
        solution_set.iter().for_each(|(var, val)| {
            println!("{}: {:?}", var, val);
        });
    });

    Ok(())
}

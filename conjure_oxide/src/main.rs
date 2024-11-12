// (niklasdewally): temporary, gut this if you want!

use std::fs::File;
use std::io::stdout;
use std::io::Write;
use std::path::PathBuf;
use std::process::exit;

use anyhow::Result as AnyhowResult;
use anyhow::{anyhow, bail};
use clap::{arg, command, Parser};
use schemars::schema_for;
use serde_json::json;
use serde_json::to_string_pretty;
use structured_logger::{json::new_writer, Builder};

use conjure_core::context::Context;
use conjure_oxide::find_conjure::conjure_executable;
use conjure_oxide::generate_custom::get_example_model;
use conjure_oxide::parse::model_from_json;
use conjure_oxide::rewrite::rewrite_model;
use conjure_oxide::solvers::FromConjureModel;
use minion_rs::ast::{Constant, Model as MinionModel, VarName};
use minion_rs::run_minion;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(
        value_name = "INPUT_ESSENCE",
        default_value = "./conjure_oxide/tests/integration/xyz/input.essence",
        help = "The input Essence file"
    )]
    input_file: PathBuf,

    #[arg(
        long,
        value_name = "EXTRA_RULE_SETS",
        help = "Names of extra rule sets to enable"
    )]
    extra_rule_sets: Vec<String>,

    #[arg(
        long,
        value_enum,
        value_name = "SOLVER",
        short = 's',
        help = "Solver family use (Minion by default)"
    )]
    solver: Option<SolverFamily>, // ToDo this should probably set the solver adapter

    // TODO: subcommands instead of these being a flag.
    #[arg(
        long,
        default_value_t = false,
        help = "Print the schema for the info JSON and exit"
    )]
    print_info_schema: bool,

    #[arg(long, help = "Save execution info as JSON to the given file-path.")]
    info_json_path: Option<PathBuf>,

    #[arg(
        long,
        short = 'o',
        help = "Save solutions to a JSON file (prints to stdin by default)"
    )]
    output: Option<PathBuf>,
}

#[allow(clippy::unwrap_used)]
pub fn main() -> AnyhowResult<()> {
    let cli = Cli::parse();

    #[allow(clippy::unwrap_used)]
    if cli.print_info_schema {
        let schema = schema_for!(Context);
        println!("{}", serde_json::to_string_pretty(&schema).unwrap());
        return Ok(());
    }

    let target_family = cli.solver.unwrap_or(SolverFamily::Minion);
    let extra_rule_sets: Vec<String> = cli.extra_rule_sets;
    let out_file: Option<File> = match &cli.output {
        None => None,
        Some(pth) => Some(
            File::options()
                .create(true)
                .truncate(true)
                .write(true)
                .open(pth)?,
        ),
    };
    #[allow(clippy::unwrap_used)]
    let log_file = File::options()
        .create(true)
        .append(true)
        .open("conjure_oxide.log")?;

    Builder::new()
        .with_target_writer("info", new_writer(stdout()))
        .with_target_writer("file", new_writer(log_file))
        .init();

    if target_family != SolverFamily::Minion {
        log::error!("Only the Minion solver is currently supported!");
        exit(1);
    }

    let rule_sets = match resolve_rule_sets(target_family, &extra_rule_sets) {
        Ok(rs) => rs,
        Err(e) => {
            log::error!("Error resolving rule sets: {}", e);
            exit(1);
        }
    };

    let pretty_rule_sets = rule_sets
        .iter()
        .map(|rule_set| rule_set.name)
        .collect::<Vec<_>>()
        .join(", ");

    println!("Enabled rule sets: [{}]", pretty_rule_sets);
    log::info!(
        target: "file",
        "Rule sets: {}",
        pretty_rule_sets
    );

    let rule_priorities = get_rule_priorities(&rule_sets)?;
    let rules_vec = get_rules_vec(&rule_priorities);

    log::info!(target: "file", 
         "Rules and priorities: {}", 
         rules_vec.iter()
            .map(|rule| format!("{}: {}", rule.name, rule_priorities.get(rule).unwrap_or(&0)))
            .collect::<Vec<_>>()
            .join(", "));

    log::info!(target: "file", "Input file: {}", cli.input_file.display());
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

    let context = Context::new_ptr(
        target_family,
        extra_rule_sets.clone(),
        rules_vec.clone(),
        rule_sets.clone(),
    );

    context.write().unwrap().file_name = Some(cli.input_file.to_str().expect("").into());

    let mut model = model_from_json(&astjson, context.clone())?;

    log::info!(target: "file", "Initial model: {}", json!(model));

    log::info!(target: "file", "Rewriting model...");
    model = rewrite_model(&model, &rule_sets)?;

    log::info!(target: "file", "Rewritten model: {}", json!(model));

    let solutions = get_minion_solutions(model)?; // ToDo we need to properly set the solver adaptor here, not hard code minion
    log::info!(target: "file", "Solutions: {}", minion_solutions_to_json(&solutions));

    let solutions_json = minion_solutions_to_json(&solutions);
    let solutions_str = to_string_pretty(&solutions_json)?;
    match out_file {
        None => {
            println!("Solutions:");
            println!("{}", solutions_str);
        }
        Some(mut outf) => {
            outf.write_all(solutions_str.as_bytes())?;
            println!(
                "Solutions saved to {:?}",
                &cli.output.unwrap().canonicalize()?
            )
        }
    }

    if let Some(path) = cli.info_json_path {
        #[allow(clippy::unwrap_used)]
        let context_obj = context.read().unwrap().clone();
        let generated_json = &serde_json::to_value(context_obj)?;
        let pretty_json = serde_json::to_string_pretty(&generated_json)?;
        File::create(path)?.write_all(pretty_json.as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_example_model_success() {
        let filename = "basic/comprehension-01-1/comprehension-01-1";
        get_example_model(filename).unwrap();
    }

    #[test]
    fn test_get_example_model_fail_empty_filename() {
        let filename = "";
        get_example_model(filename).unwrap_err();
    }
}

// (niklasdewally): temporary, gut this if you want!

use std::fs::File;
use std::io::stdout;
use std::io::Write;
use std::path::PathBuf;
use std::process::exit;

use anyhow::Result as AnyhowResult;
use anyhow::{anyhow, bail};
use clap::{arg, command, Parser};
use conjure_oxide::utils::json::sort_json_object;
use schemars::schema_for;
use serde_json::json;
use serde_json::to_string_pretty;
use structured_logger::{json::new_writer, Builder};

use conjure_core::context::Context;
use conjure_oxide::find_conjure::conjure_executable;
use conjure_oxide::model_from_json;
use conjure_oxide::rule_engine::{
    get_rule_priorities, get_rules_vec, resolve_rule_sets, rewrite_model,
};
use conjure_oxide::utils::conjure::{get_minion_solutions, minion_solutions_to_json};
use conjure_oxide::SolverFamily;

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

    // TODO: subcommands instead of these being a flag.
    #[arg(long, default_value_t = false)]
    /// Prints the schema for the info JSON then exits.
    print_info_schema: bool,

    #[arg(long)]
    /// Saves execution info as JSON to the given file-path.
    info_json_path: Option<PathBuf>,
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

    let target_family = SolverFamily::Minion; // ToDo get this from CLI input
    let extra_rule_sets: Vec<&str> = vec!["Constant"]; // ToDo get this from CLI input

    #[allow(clippy::unwrap_used)]
    let log_file = File::options()
        .create(true)
        .append(true)
        .open("conjure_oxide.log")
        .unwrap();

    Builder::new()
        .with_target_writer("info", new_writer(stdout()))
        .with_target_writer("file", new_writer(log_file))
        .init();

    let rule_sets = match resolve_rule_sets(target_family, &extra_rule_sets) {
        Ok(rs) => rs,
        Err(e) => {
            log::error!("Error resolving rule sets: {}", e);
            exit(1);
        }
    };

    log::info!(
        target: "info",
        "Rule sets: {}",
        rule_sets.iter().map(|rule_set| rule_set.name).collect::<Vec<_>>().join(", ")
    );

    let rule_priorities = get_rule_priorities(&rule_sets)?;
    let rules_vec = get_rules_vec(&rule_priorities);

    log::info!(target: "info", 
         "Rules and priorities: {}", 
         rules_vec.iter()
            .map(|rule| format!("{}: {}", rule.name, rule_priorities.get(rule).unwrap_or(&0)))
            .collect::<Vec<_>>()
            .join(", "));

    log::info!("Input file: {}", cli.input_file.display());
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

    log::info!("Initial model: {}", to_string_pretty(&json!(model))?);

    log::info!("Rewriting model...");
    model = rewrite_model(&model, &rule_sets)?;

    log::info!("Rewritten model: {}", to_string_pretty(&json!(model))?);
    println!("\nRewritten model:");
    println!("{:#?}", model); 

    let solutions = get_minion_solutions(model)?;
    log::info!("Solutions: {}", minion_solutions_to_json(&solutions));

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
    use conjure_oxide::{get_example_model, get_example_model_by_path};

    #[test]
    fn test_get_example_model_success() {
        let filename = "input";
        get_example_model(filename).unwrap();
    }

    #[test]
    fn test_get_example_model_by_filepath() {
        let filepath = "tests/integration/xyz/input.essence";
        get_example_model_by_path(filepath).unwrap();
    }

    #[test]
    fn test_get_example_model_fail_empty_filename() {
        let filename = "";
        get_example_model(filename).unwrap_err();
    }

    #[test]
    fn test_get_example_model_fail_empty_filepath() {
        let filepath = "";
        get_example_model_by_path(filepath).unwrap_err();
    }
}

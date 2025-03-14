use anyhow::Result as AnyhowResult;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;

use anyhow::{anyhow, bail};

use clap::{arg, command, Parser};

use schemars::schema_for;

// use conjure_oxide::defaults::get_default_rule_sets;
use conjure_oxide::defaults::DEFAULT_RULE_SETS;
use conjure_oxide::find_conjure::conjure_executable;
use conjure_oxide::rule_engine::{resolve_rule_sets, rewrite_model};
use conjure_oxide::utils::conjure::{get_minion_solutions, get_sat_solutions, solutions_to_json};
use conjure_oxide::{get_rules, model_from_json, SolverFamily};

use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Layer};

use conjure_core::context::Context;
use conjure_core::rule_engine::rewrite_naive;
use conjure_core::Model;

use git_version::git_version;

use serde_json::to_string_pretty;

static AFTER_HELP_TEXT: &str = include_str!("help_text.txt");

#[derive(Parser, Clone)]
#[command(author, about, long_about = None, after_long_help=AFTER_HELP_TEXT)]
struct Cli {
    #[arg(value_name = "INPUT_ESSENCE", help = "The input Essence file")]
    input_file: Option<PathBuf>,

    #[arg(
        long,
        value_name = "EXTRA_RULE_SETS",
        help = "Names of extra rule sets to enable"
    )]
    extra_rule_sets: Vec<String>,

    #[arg(
        long = "solver",
        value_enum,
        value_name = "SOLVER",
        short = 's',
        help = "Solver family to use (Minion by default)"
    )]
    solver: Option<SolverFamily>, // ToDo this should probably set the solver adapter

    #[arg(
        long,
        default_value_t = 0,
        short = 'n',
        help = "number of solutions to return (0 for all)"
    )]
    number_of_solutions: i32,
    // TODO: subcommands instead of these being a flag.
    #[arg(
        long,
        default_value_t = false,
        help = "Print the schema for the info JSON and exit"
    )]
    print_info_schema: bool,

    #[arg(
        long = "version",
        short = 'V',
        help = "Print the version of the program (git commit) and exit"
    )]
    version: bool,

    #[arg(long, help = "Save execution info as JSON to the given file-path.")]
    info_json_path: Option<PathBuf>,

    #[arg(
        long,
        help = "use the, in development, dirty-clean optimising rewriter",
        default_value_t = false
    )]
    use_optimising_rewriter: bool,

    #[arg(
        long,
        short = 'o',
        help = "Save solutions to a JSON file (prints to stdout by default)"
    )]
    output: Option<PathBuf>,

    #[arg(long, short = 'v', help = "Log verbosely to sterr")]
    verbose: bool,

    /// Do not run the solver.
    ///
    /// The rewritten model is printed to stdout in an Essence-style syntax (but is not necessarily
    /// valid Essence).
    #[arg(long, default_value_t = false)]
    no_run_solver: bool,

    // --no-x flag disables --x flag : https://jwodder.github.io/kbits/posts/clap-bool-negate/
    /// Check for multiple equally applicable rules, exiting if any are found.
    ///
    /// Only compatible with the default rewriter.
    #[arg(
        long,
        overrides_with = "_no_check_equally_applicable_rules",
        default_value_t = false
    )]
    check_equally_applicable_rules: bool,

    /// Do not check for multiple equally applicable rules [default].
    ///
    /// Only compatible with the default rewriter.
    #[arg(long)]
    _no_check_equally_applicable_rules: bool,
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
    let mut extra_rule_sets: Vec<&str> = DEFAULT_RULE_SETS.to_vec();
    for rs in &cli.extra_rule_sets {
        extra_rule_sets.push(rs.as_str());
    }

    // Logging:
    //
    // Using `tracing` framework, but this automatically reads stuff from `log`.
    //
    // A Subscriber is responsible for logging.
    //
    // It consists of composable layers, each of which logs to a different place in a different
    // format.
    let json_log_file = File::options()
        .create(true)
        .append(true)
        .open("conjure_oxide_log.json")?;

    let log_file = File::options()
        .create(true)
        .append(true)
        .open("conjure_oxide.log")?;

    // get log level from env-var RUST_LOG

    let json_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(Arc::new(json_log_file))
        .with_filter(LevelFilter::TRACE);

    let file_layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_ansi(false)
        .with_writer(Arc::new(log_file))
        .with_filter(LevelFilter::TRACE);

    let default_stderr_level = if cli.verbose {
        LevelFilter::DEBUG
    } else {
        LevelFilter::WARN
    };

    let env_filter = EnvFilter::builder()
        .with_default_directive(default_stderr_level.into())
        .from_env_lossy();

    let stderr_layer = if cli.verbose {
        Layer::boxed(
            tracing_subscriber::fmt::layer()
                .pretty()
                .with_writer(Arc::new(std::io::stderr()))
                .with_ansi(true)
                .with_filter(env_filter),
        )
    } else {
        Layer::boxed(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_writer(Arc::new(std::io::stderr()))
                .with_ansi(true)
                .with_filter(env_filter),
        )
    };

    if cli.version {
        println!("Version: {}", git_version!());
        return Ok(());
    }

    // load the loggers
    tracing_subscriber::registry()
        .with(json_layer)
        .with(stderr_layer)
        .with(file_layer)
        .init();

    if !(target_family == SolverFamily::SAT || target_family == SolverFamily::Minion) {
        tracing::error!("Only the SAT and Minion solver is currently supported!");
        exit(1);
    }

    let rule_sets = match resolve_rule_sets(target_family, &extra_rule_sets) {
        Ok(rs) => rs,
        Err(e) => {
            tracing::error!("Error resolving rule sets: {}", e);
            exit(1);
        }
    };

    let pretty_rule_sets = rule_sets
        .iter()
        .map(|rule_set| rule_set.name)
        .collect::<Vec<_>>()
        .join(", ");

    tracing::info!("Enabled rule sets: [{}]", pretty_rule_sets);
    tracing::info!(
        target: "file",
        "Rule sets: {}",
        pretty_rule_sets
    );

    let rules = get_rules(&rule_sets)?.into_iter().collect::<Vec<_>>();
    tracing::info!(
        target: "file",
        "Rules: {}",
        rules.iter().map(|rd| format!("{}", rd)).collect::<Vec<_>>().join("\n")
    );
    let input = cli.input_file.clone().expect("No input file given");
    tracing::info!(target: "file", "Input file: {}", input.display());
    let input_file: &str = input.to_str().ok_or(anyhow!(
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
        extra_rule_sets.iter().map(|rs| rs.to_string()).collect(),
        rules,
        rule_sets.clone(),
    );

    context.write().unwrap().file_name = Some(input.to_str().expect("").into());

    if cfg!(feature = "extra-rule-checks") {
        tracing::info!("extra-rule-checks: enabled");
    } else {
        tracing::info!("extra-rule-checks: disabled");
    }

    let mut model = model_from_json(&astjson, context.clone())?;

    tracing::info!("Initial model: \n{}\n", model);

    tracing::info!("Rewriting model...");

    if cli.use_optimising_rewriter {
        tracing::info!("Using the dirty-clean rewriter...");
        model = rewrite_model(&model, &rule_sets)?;
    } else {
        tracing::info!("Rewriting model...");
        model = rewrite_naive(&model, &rule_sets, cli.check_equally_applicable_rules)?;
    }

    tracing::info!("Rewritten model: \n{}\n", model);

    if cli.no_run_solver {
        println!("{}", model);
    } else {
        run_solver(&cli.clone(), model)?;
    }

    // still do postamble even if we didn't run the solver
    if let Some(path) = cli.info_json_path {
        #[allow(clippy::unwrap_used)]
        let context_obj = context.read().unwrap().clone();
        let generated_json = &serde_json::to_value(context_obj)?;
        let pretty_json = serde_json::to_string_pretty(&generated_json)?;
        File::create(path)?.write_all(pretty_json.as_bytes())?;
    }
    Ok(())
}

/// Runs the solver
fn run_solver(cli: &Cli, model: Model) -> anyhow::Result<()> {
    let solver = cli.solver;
    match solver {
        Some(sol_family) => match sol_family {
            SolverFamily::SAT => run_sat_solver(cli, model),
            SolverFamily::Minion => run_minion(cli, model),
        },
        None => panic!("main::run_solver() : Unreachable: Should never be None"),
    }
}

fn run_minion(cli: &Cli, model: Model) -> anyhow::Result<()> {
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

    let solutions = get_minion_solutions(model, cli.number_of_solutions)?;
    tracing::info!(target: "file", "Solutions: {}", solutions_to_json(&solutions));

    let solutions_json = solutions_to_json(&solutions);
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
                &cli.output.clone().unwrap().canonicalize()?
            )
        }
    }
    Ok(())
}

fn run_sat_solver(cli: &Cli, model: Model) -> anyhow::Result<()> {
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

    let solutions = get_sat_solutions(model, cli.number_of_solutions)?;
    tracing::info!(target: "file", "Solutions: {}", solutions_to_json(&solutions));

    let solutions_json = solutions_to_json(&solutions);
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
                &cli.output.clone().unwrap().canonicalize()?
            )
        }
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

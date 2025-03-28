#![allow(clippy::unwrap_used)]
mod cli;
mod print_info_schema;
mod solve;
mod test_solve;
use clap::Parser as _;
use cli::{Cli, GlobalArgs};
use conjure_oxide::SolverFamily;
use print_info_schema::run_print_info_schema_command;
use solve::run_solve_command;
use std::fs::File;
use std::process::exit;
use std::sync::Arc;
use test_solve::run_test_solve_command;

use anyhow::Result as AnyhowResult;
use anyhow::{anyhow, bail};
use clap::{arg, command, Parser};
use conjure_core::pro_trace::{
    self, create_consumer, specify_trace_file, Consumer, FileConsumer, HumanFormatter,
    JsonFormatter, StdoutConsumer, VerbosityLevel,
};
use git_version::git_version;
use tracing_subscriber::filter::{FilterFn, LevelFilter};
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{fmt, EnvFilter, Layer};

pub fn main() {
    // exit with 2 instead of 1 on failure,like grep
    match run() {
        Ok(_) => {
            exit(0);
        }
        Err(e) => {
            eprintln!("{:?}", e);
            exit(2);
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.version {
        println!("Version: {}", git_version!());
        return Ok(());
    }

    setup_logging(&cli.global_args)?;

    run_subcommand(cli)
}

fn setup_logging(global_args: &GlobalArgs) -> anyhow::Result<()> {
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

    let default_stderr_level = if global_args.verbose {
        LevelFilter::DEBUG
    } else {
        LevelFilter::WARN
    };

    let env_filter = EnvFilter::builder()
        .with_default_directive(default_stderr_level.into())
        .from_env_lossy();

    let stderr_layer = if global_args.verbose {
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

    let human_rule_trace_layer = global_args.human_rule_trace.clone().map(|x| {
        let file = File::create(x).expect("Unable to create rule trace file");
        fmt::layer()
            .with_writer(file)
            .with_level(false)
            .without_time()
            .with_target(false)
            .with_filter(EnvFilter::new("rule_engine_human=trace"))
            .with_filter(FilterFn::new(|meta| meta.target() == "rule_engine_human"))
    });
    // load the loggers
    tracing_subscriber::registry()
        .with(json_layer)
        .with(stderr_layer)
        .with(file_layer)
        .with(human_rule_trace_layer)
        .init();

    if target_family != SolverFamily::Minion {
        tracing::error!("Only the Minion solver is currently supported!");
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
        rules.iter().map(|rd| format!("{}", rd )).collect::<Vec<_>>().join("\n")
    );
    let input = cli.input_file.clone().expect("No input file given");
    tracing::info!(target: "file", "Input file: {}", input.display());
    let input_file: &str = input.to_str().ok_or(anyhow!(
        "Given input_file could not be converted to a string"
    ))?;

    let file = specify_trace_file(
        input_file.to_string(),
        cli.trace_file.clone(),
        cli.formatter.as_str(),
    );
    //consumer for protrace
    let consumer: Option<Consumer> = cli.tracing.then(|| {
        create_consumer(
            cli.trace_output.as_str(),
            cli.verbosity.clone(),
            cli.formatter.as_str(),
            file,
        )
    });
    /******************************************************/
    /*        Parse essence to json using Conjure         */
    /******************************************************/

    let context = Context::new_ptr(
        target_family,
        extra_rule_sets.iter().map(|rs| rs.to_string()).collect(),
        rules,
        rule_sets.clone(),
    );
    context.write().unwrap().file_name = Some(input.to_str().expect("").into());

    let mut model;
    if cli.enable_native_parser {
        model = parse_essence_file_native(input_file, context.clone())?;
    } else {
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

        if cfg!(feature = "extra-rule-checks") {
            tracing::info!("extra-rule-checks: enabled");
        } else {
            tracing::info!("extra-rule-checks: disabled");
        }

        model = model_from_json(&astjson, context.clone())?;
    }

    tracing::info!("Initial model: \n{}\n", model);

    tracing::info!("Rewriting model...");

    if !cli.use_optimising_rewriter {
        tracing::info!("Rewriting model...");
        model = rewrite_naive(
            &model,
            &rule_sets,
            cli.check_equally_applicable_rules,
            consumer,
        )?;
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

/// Runs the selected subcommand
fn run_subcommand(cli: Cli) -> anyhow::Result<()> {
    let global_args = cli.global_args;
    match cli.subcommand {
        cli::Command::Solve(solve_args) => run_solve_command(global_args, solve_args),
        cli::Command::TestSolve(local_args) => run_test_solve_command(global_args, local_args),
        cli::Command::PrintJsonSchema => run_print_info_schema_command(),
    }
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

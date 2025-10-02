#![allow(clippy::unwrap_used)]
mod cli;
mod print_info_schema;
mod solve;
mod test_solve;
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use cli::{Cli, GlobalArgs};
use print_info_schema::run_print_info_schema_command;
use solve::run_solve_command;
use std::fs::File;
use std::io;
use std::process::exit;
use std::sync::Arc;
use test_solve::run_test_solve_command;

use conjure_cp_rules as _;

use git_version::git_version;
use tracing_subscriber::filter::{FilterFn, LevelFilter};
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Layer, fmt};

pub fn main() {
    // exit with 2 instead of 1 on failure,like grep
    match run() {
        Ok(_) => {
            exit(0);
        }
        Err(e) => {
            eprintln!("{e:?}");
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

    Ok(())
}

fn run_completion_command(completion_args: cli::CompletionArgs) -> anyhow::Result<()> {
    let mut cmd = Cli::command();
    let shell = completion_args.shell;
    let name = cmd.get_name().to_string();

    eprintln!("Generating completion for {shell}...");

    generate(shell, &mut cmd, name, &mut io::stdout());
    Ok(())
}

/// Runs the selected subcommand
fn run_subcommand(cli: Cli) -> anyhow::Result<()> {
    let global_args = cli.global_args;
    match cli.subcommand {
        cli::Command::Solve(solve_args) => run_solve_command(global_args, solve_args),
        cli::Command::TestSolve(local_args) => run_test_solve_command(global_args, local_args),
        cli::Command::PrintJsonSchema => run_print_info_schema_command(),
        cli::Command::Completion(completion_args) => run_completion_command(completion_args),
    }
}

#[cfg(test)]
mod tests {
    use conjure_cp::parse::conjure_json::{get_example_model, get_example_model_by_path};

    #[test]
    fn test_get_example_model_success() {
        let filename = "input";
        get_example_model(filename).unwrap();
    }

    #[test]
    fn test_get_example_model_by_filepath() {
        let filepath = "../../tests-integration/tests/integration/xyz/input.essence";
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

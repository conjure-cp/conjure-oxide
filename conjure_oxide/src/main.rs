mod cli;
mod print_info_schema;
mod solve;
use clap::Parser as _;
use cli::{Cli, GlobalArgs};
use print_info_schema::run_print_info_schema_command;
use solve::run_solve_command;
use std::fs::File;
use std::sync::Arc;

use git_version::git_version;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Layer};

#[allow(clippy::unwrap_used)]
pub fn main() -> anyhow::Result<()> {
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

    // load the loggers
    tracing_subscriber::registry()
        .with(json_layer)
        .with(stderr_layer)
        .with(file_layer)
        .init();

    Ok(())
}

/// Runs the selected subcommand
fn run_subcommand(cli: Cli) -> anyhow::Result<()> {
    match cli.subcommand {
        cli::Command::Solve(solve_args) => run_solve_command(cli.global_args, solve_args),
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

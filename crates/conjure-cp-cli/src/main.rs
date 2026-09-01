#![allow(clippy::unwrap_used)]

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod cli;
mod pretty;
mod print_info_schema;
mod rule_trace_aggregates;
mod solve;
mod test_solve;
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use cli::{Cli, GlobalArgs, LogDetail, LogFormat};
use pretty::run_pretty_command;
use print_info_schema::run_print_info_schema_command;
use rule_trace_aggregates::RuleTraceAggregatesHandle;
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

use conjure_cp_lsp::server;

struct LoggingState {
    rule_trace_aggregates: Option<RuleTraceAggregatesHandle>,
}

impl LoggingState {
    fn flush(&self) {
        if let Some(handle) = &self.rule_trace_aggregates {
            handle.flush();
        }
    }
}

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

    let logging_state = setup_logging(&cli.global_args)?;
    let result = run_subcommand(cli);
    logging_state.flush();
    result
}

fn setup_logging(global_args: &GlobalArgs) -> anyhow::Result<LoggingState> {
    // It consists of composable layers, each of which logs to a different place in a different
    // format.
    let default_stderr_level = match (global_args.quiet, global_args.verbose) {
        (true, _) => LevelFilter::OFF,
        (false, 0) => LevelFilter::WARN,
        (false, 1) => LevelFilter::INFO,
        (false, 2) => LevelFilter::DEBUG,
        (false, _) => LevelFilter::TRACE,
    };

    let env_filter = if global_args.quiet || global_args.verbose > 0 {
        EnvFilter::new(default_stderr_level.to_string())
    } else {
        EnvFilter::builder()
            .with_default_directive(default_stderr_level.into())
            .from_env_lossy()
    };

    let stderr_layer = if global_args.verbose > 0 {
        Layer::boxed(
            tracing_subscriber::fmt::layer()
                .pretty()
                .with_writer(Arc::new(std::io::stderr()))
                .with_ansi(true)
                .with_filter(env_filter)
                .with_filter(general_log_filter()),
        )
    } else {
        Layer::boxed(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_writer(Arc::new(std::io::stderr()))
                .with_ansi(true)
                .with_filter(env_filter)
                .with_filter(general_log_filter()),
        )
    };

    let rule_trace_layer = global_args.rule_trace.clone().map(|x| {
        let file = File::create(x).expect("Unable to create rule trace file");
        fmt::layer()
            .with_writer(file)
            .with_level(false)
            .without_time()
            .with_target(false)
            .with_filter(EnvFilter::new("rule_engine_rule_trace=trace"))
            .with_filter(FilterFn::new(|meta| {
                meta.target() == "rule_engine_rule_trace"
            }))
    });

    let rule_attempt_trace_layer = global_args.rule_attempt_trace.clone().map(|x| {
        let file = File::create(x).expect("Unable to create rule attempt trace file");
        fmt::layer()
            .with_writer(file)
            .with_level(false)
            .without_time()
            .with_target(false)
            .compact()
            .with_ansi(false)
            .with_filter(EnvFilter::new("rule_engine_rule_attempt_trace=trace"))
            .with_filter(FilterFn::new(|meta| {
                meta.target() == "rule_engine_rule_attempt_trace"
            }))
    });

    let rule_trace_aggregates_handle = global_args
        .rule_trace_aggregates
        .clone()
        .map(RuleTraceAggregatesHandle::new)
        .transpose()?;

    let rule_trace_aggregates_layer = rule_trace_aggregates_handle.as_ref().map(|handle| {
        handle
            .layer()
            .with_filter(EnvFilter::new("rule_engine_rule_trace_aggregates=trace"))
            .with_filter(FilterFn::new(|meta| {
                meta.target() == "rule_engine_rule_trace_aggregates"
            }))
    });

    let log_format = global_args.log_format.unwrap_or_default();
    let log_detail = global_args.log_detail.unwrap_or_default();
    let text_file = match (log_format, global_args.log_file.as_ref()) {
        (LogFormat::Text, Some(path)) => Some(
            File::options()
                .truncate(true)
                .write(true)
                .create(true)
                .append(false)
                .open(path)?,
        ),
        _ => None,
    };

    let file_level = match log_detail {
        LogDetail::Stages => LevelFilter::INFO,
        LogDetail::Applications => LevelFilter::DEBUG,
        LogDetail::Attempts => LevelFilter::TRACE,
    };
    let text_file_layer = text_file.map(|file| {
        tracing_subscriber::fmt::layer()
            .compact()
            .with_ansi(false)
            .with_writer(Arc::new(file))
            .with_filter(file_level)
            .with_filter(general_log_filter())
    });
    let json_file = match (log_format, global_args.log_file.as_ref()) {
        (LogFormat::Json, Some(path)) => Some(
            File::options()
                .truncate(true)
                .write(true)
                .create(true)
                .append(false)
                .open(path)?,
        ),
        _ => None,
    };
    let json_file_layer = json_file.map(|file| {
        tracing_subscriber::fmt::layer()
            .json()
            .with_writer(Arc::new(file))
            .with_filter(file_level)
            .with_filter(general_log_filter())
    });

    tracing_subscriber::registry()
        .with(stderr_layer)
        .with(rule_trace_layer)
        .with(rule_attempt_trace_layer)
        .with(rule_trace_aggregates_layer)
        .with(text_file_layer)
        .with(json_file_layer)
        .init();

    Ok(LoggingState {
        rule_trace_aggregates: rule_trace_aggregates_handle,
    })
}

fn general_log_filter() -> FilterFn<fn(&tracing::Metadata<'_>) -> bool> {
    fn is_general_log(meta: &tracing::Metadata<'_>) -> bool {
        !meta.target().starts_with("rule_engine_rule_")
    }

    FilterFn::new(is_general_log)
}

fn run_completion_command(completion_args: cli::CompletionArgs) -> anyhow::Result<()> {
    let mut cmd = Cli::command();
    let shell = completion_args.shell;
    let name = cmd.get_name().to_string();

    eprintln!("Generating completion for {shell}...");

    generate(shell, &mut cmd, name, &mut io::stdout());
    Ok(())
}

fn run_lsp_server() -> anyhow::Result<()> {
    server::main();
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
        cli::Command::Pretty(pretty_args) => run_pretty_command(global_args, pretty_args),
        cli::Command::ServerLSP => run_lsp_server(),
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
        let filepath = "../../test-suite/tests/integration/basic/misc/xyz/input.essence";
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

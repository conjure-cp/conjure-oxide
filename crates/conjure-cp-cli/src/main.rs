#![allow(clippy::unwrap_used)]

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod cli;
mod pretty;
mod rule_trace_aggregates;
mod solve;
mod test_solve;
use anyhow::Context as _;
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use cli::{Cli, GlobalArgs, LogDetail, LogFormat};
use conjure_cp_cli::utils::rule_trace::RuleApplicationCounter;
use pretty::run_pretty_command;
use rule_trace_aggregates::write_rule_trace_aggregates;
use solve::run_solve_command;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use test_solve::run_test_solve_command;

use conjure_cp_rules as _;

use tracing_subscriber::filter::{FilterFn, LevelFilter};
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Layer, fmt};

#[cfg(feature = "lsp")]
use conjure_cp_lsp::server;

struct LoggingState {
    rule_application_counts: RuleApplicationCounter,
    rule_trace_aggregates_path: Option<PathBuf>,
}

impl LoggingState {
    fn flush(&self) {
        if let Some(path) = &self.rule_trace_aggregates_path {
            write_rule_trace_aggregates(path, &self.rule_application_counts.snapshot())
                .expect("failed to write rule trace aggregates");
        }
    }
}

static LOGGING_STATE: OnceLock<LoggingState> = OnceLock::new();

/// Writes buffered log outputs, such as the final rule-trace aggregates.
fn flush_logging() {
    if let Some(state) = LOGGING_STATE.get() {
        state.flush();
    }
}

/// Flushes buffered log outputs, then exits. Use instead of [`std::process::exit`].
pub(crate) fn exit(code: i32) -> ! {
    flush_logging();
    std::process::exit(code)
}

/// Flushes on return and while unwinding from a panic.
struct FlushLoggingOnDrop;

impl Drop for FlushLoggingOnDrop {
    fn drop(&mut self) {
        flush_logging();
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

    let logging_state = setup_logging(&cli.global_args)?;
    let rule_application_counts = logging_state.rule_application_counts.clone();
    let _ = LOGGING_STATE.set(logging_state);
    let _flush = FlushLoggingOnDrop;
    run_subcommand(cli, &rule_application_counts)
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

    // Validate an explicitly requested output before rewriting. Collection itself
    // always happens in memory, independently of the output destination.
    if let Some(path) = &global_args.rule_trace_aggregates {
        File::create(path)
            .with_context(|| format!("Unable to create aggregate trace file {}", path.display()))?;
    }

    let rule_application_counts = RuleApplicationCounter::default();
    let rule_trace_aggregates_layer = rule_application_counts
        .clone()
        .with_filter(EnvFilter::new("rule_engine_rule_trace_aggregates=trace"))
        .with_filter(FilterFn::new(|meta| {
            meta.target() == "rule_engine_rule_trace_aggregates"
        }));

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
        rule_application_counts,
        rule_trace_aggregates_path: global_args.rule_trace_aggregates.clone(),
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
    #[cfg(feature = "lsp")]
    {
        server::main();
        Ok(())
    }
    #[cfg(not(feature = "lsp"))]
    anyhow::bail!("Language server support was not compiled in (enable the `lsp` feature).");
}

/// Runs the selected subcommand
fn run_subcommand(
    cli: Cli,
    rule_application_counts: &RuleApplicationCounter,
) -> anyhow::Result<()> {
    let global_args = cli.global_args;
    match cli.subcommand {
        cli::Command::Solve(solve_args) => {
            run_solve_command(global_args, solve_args, rule_application_counts)
        }
        cli::Command::TestSolve(local_args) => run_test_solve_command(global_args, local_args),
        cli::Command::Completion(completion_args) => run_completion_command(completion_args),
        cli::Command::Pretty(pretty_args) => run_pretty_command(global_args, pretty_args),
        cli::Command::ServerLSP => run_lsp_server(),
    }
}

#[cfg(test)]
mod tests {
    use conjure_cp::parse::conjure_json::{get_example_model, get_example_model_by_path};

    #[cfg(not(feature = "lsp"))]
    #[test]
    fn language_server_without_support_reports_the_missing_feature() {
        let error = super::run_lsp_server().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Language server support was not compiled in")
        );
    }

    #[test]
    fn test_get_example_model_success() {
        let filename = "input";
        get_example_model(filename).unwrap();
    }

    #[test]
    fn test_get_example_model_by_filepath() {
        let filepath = "../../test-suite/tests/exhaustive/basic/misc/xyz/input.essence";
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

use std::path::PathBuf;

use clap::{arg, command, Args, Parser, Subcommand};

use clap_complete::Shell;
use conjure_core::pro_trace::{Kind, VerbosityLevel};
use conjure_oxide::SolverFamily;

use crate::{solve, test_solve};
static AFTER_HELP_TEXT: &str = include_str!("help_text.txt");

// use once_cell::sync::Lazy;
// use std::sync::Mutex;

// pub static KIND_FILTER: Lazy<Mutex<Option<Kind>>> = Lazy::new(|| Mutex::new(None))

/// All subcommands of conjure-oxide
#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    /// Solve a model
    Solve(solve::Args),
    /// Print the JSON info file schema
    PrintJsonSchema,
    /// Tests whether the Essence model is solvable with Conjure Oxide, and whether it gets the
    /// same solutions as Conjure.
    ///
    /// Return-code will be 0 if the solutions match, 1 if they don't, and >1 on crash.
    TestSolve(test_solve::Args),
    /// Generate a completion script for the shell provided
    Completion(CompletionArgs),
}

/// Global command line arguments.
#[derive(Clone, Debug, Parser)]
#[command(author, about, long_about = None, after_long_help=AFTER_HELP_TEXT)]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: Command,

    #[command(flatten)]
    pub global_args: GlobalArgs,

    /// Print version
    #[arg(long = "version", short = 'V')]
    pub version: bool,
}

#[derive(Debug, Clone, Args)]
pub struct GlobalArgs {
    /// Extra rule sets to enable
    #[arg(long, value_name = "EXTRA_RULE_SETS", global = true)]
    pub extra_rule_sets: Vec<String>,

    /// Solver family to use
    /// By Default, Conjure Oxide will use the Minion solver.
    #[arg(
        long,
        value_enum,
        value_name = "SOLVER",
        default_value = "Minion",
        short = 's',
        global = true
    )]
    pub solver: SolverFamily,

    /// Log verbosely
    #[arg(long, short = 'v', help = "Log verbosely to sterr", global = true)]
    pub verbose: bool,

    // --no-x flag disables --x flag : https://jwodder.github.io/kbits/posts/clap-bool-negate/
    /// Check for multiple equally applicable rules, exiting if any are found.
    ///
    /// Only compatible with the default rewriter.
    #[arg(
        long,
        overrides_with = "_no_check_equally_applicable_rules",
        default_value_t = false,
        global = true
    )]
    pub check_equally_applicable_rules: bool,

    /// Output file for the human readable rule trace.
    #[arg(long, global = true)]
    pub human_rule_trace: Option<PathBuf>,

    /// Do not check for multiple equally applicable rules [default].
    ///
    /// Only compatible with the default rewriter.
    #[arg(long, global = true)]
    pub _no_check_equally_applicable_rules: bool,

    /// Use the native parser instead of Conjure's.
    #[arg(long, default_value_t = false, global = true)]
    pub enable_native_parser: bool,

    // New logging arguments:
    // Tracing: T
    // Output: stdout, file
    // Verbosity: low, medium, high
    // Format: human, json, both (in case of file)
    // Optional file path
    #[arg(
        long,
        short = 'T',
        default_value_t = false,
        global = true,
        help = "Enable rule tracing"
    )]
    pub tracing: bool,

    #[arg(
        long,
        short = 'O',
        default_value = "stdout",
        global = true,
        help = "Select output location for trace result: stdout or file"
    )]
    pub trace_output: String,

    #[arg(
        long,
        default_value = "medium",
        global = true,
        help = "Select verbosity level for trace"
    )]
    pub verbosity: VerbosityLevel,

    #[arg(
        long,
        short = 'F',
        default_value = "human",
        global = true,
        help = "Select the format of the trace output: human or json"
    )]
    pub formatter: String,

    // Can optionally specify where the trace should be located by specifying the file path
    // The flag accepts
    #[arg(
        long,
        short = 'f',
        global = true,
        num_args = 0..=2,
        value_names = ["JSON_TRACE", "HUMAN_TRACE"],
        help = "Optionally save traces to specific files: first for JSON, second for human format"
    )]
    pub trace_file: Option<Vec<String>>,

    #[arg(
        long = "get-info-about",
        global = true,
        value_delimiter = ' ',
        help = "Filter messages by given kind"
    )]
    pub kind_filter: Option<Kind>,

    #[arg(
        long = "filter-rule-name",
        global = true,
        help = "Filter rule trace to only show given rule name"
    )]
    pub rule_name_filter: Option<String>,

    #[arg(
        long = "filter-rule-set",
        global = true,
        help = "Filter rule trace to only show given rule set"
    )]
    pub rule_set_filter: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct CompletionArgs {
    /// Shell type for which to generate the completion script
    #[arg(value_enum)]
    pub shell: Shell,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ShellTypes {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Elvish,
}

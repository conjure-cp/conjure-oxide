use clap::{arg, command, Args, Parser, Subcommand};

use conjure_oxide::SolverFamily;

use crate::solve;
static AFTER_HELP_TEXT: &str = include_str!("help_text.txt");

/// All subcommands of conjure-oxide
#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    /// Solve a model
    Solve(solve::Args),
    /// Print the JSON info file schema
    PrintJsonSchema,
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
    #[arg(long, value_enum, value_name = "SOLVER", short = 's', global = true)]
    pub solver: Option<SolverFamily>,

    /// Use the in development dirty-clean optimising rewriter
    #[arg(long, default_value_t = false, global = true)]
    pub use_optimising_rewriter: bool,

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

    /// Do not check for multiple equally applicable rules [default].
    ///
    /// Only compatible with the default rewriter.
    #[arg(long, global = true)]
    pub _no_check_equally_applicable_rules: bool,

    /// Use the native parser instead of Conjure's.
    #[arg(long, default_value_t = false, global = true)]
    pub enable_native_parser: bool,
}

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, arg, command};

use clap_complete::Shell;
use conjure_cp::solver::SolverFamily;

use crate::{solve, test_solve};

pub(crate) const DEBUG_HELP_HEADING: Option<&str> = Some("Debug");
pub(crate) const LOGGING_HELP_HEADING: Option<&str> = Some("Logging & Output");
pub(crate) const EXPERIMENTAL_HELP_HEADING: Option<&str> = Some("Experimental");
pub(crate) const OPTIMISATIONS_HELP_HEADING: Option<&str> = Some("Optimisations");

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
#[command(
    author,
    about = "Conjure Oxide: Automated Constraints Modelling Toolkit",
    before_help = "Full documentation can be found online at: https://conjure-cp.github.io/conjure-oxide"
)]
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
    #[arg(
        long,
        value_enum,
        value_name = "SOLVER",
        default_value_t = SolverFamily::Minion,
        short = 's',
        global = true
    )]
    pub solver: SolverFamily,

    /// Log verbosely
    #[arg(long, short = 'v', help = "Log verbosely to sterr", global = true, help_heading = LOGGING_HELP_HEADING)]
    pub verbose: bool,

    // --no-x flag disables --x flag : https://jwodder.github.io/kbits/posts/clap-bool-negate/
    /// Check for multiple equally applicable rules, exiting if any are found.
    ///
    /// Only compatible with the default rewriter.
    #[arg(
        long,
        overrides_with = "_no_check_equally_applicable_rules",
        default_value_t = false,
        global = true,
        help_heading= DEBUG_HELP_HEADING
    )]
    pub check_equally_applicable_rules: bool,

    /// Output file for the human readable rule trace.
    #[arg(long, global = true, help_heading=LOGGING_HELP_HEADING)]
    pub human_rule_trace: Option<PathBuf>,

    /// Do not check for multiple equally applicable rules [default].
    ///
    /// Only compatible with the default rewriter.
    #[arg(long, global = true, help_heading = DEBUG_HELP_HEADING)]
    pub _no_check_equally_applicable_rules: bool,

    /// Use the native parser instead of Conjure's.
    #[arg(long, default_value_t = false, global = true, help_heading = EXPERIMENTAL_HELP_HEADING)]
    pub use_native_parser: bool,

    /// Do not use better comprehension expanding for ac operators (and,or,sum,product).
    #[arg(long, default_value_t = false, global=true, help_heading = OPTIMISATIONS_HELP_HEADING)]
    pub no_use_expand_ac: bool,

    /// Save a solver input file to <filename>.
    ///
    /// This input file will be in a format compatible by the command-line
    /// interface of the selected solver. For example, when the solver is Minion,
    /// a valid .minion file will be output.
    ///
    /// This file is for informational purposes only; the results of running
    /// this file cannot be used by Conjure Oxide in any way.
    #[arg(long,global=true, value_names=["filename"], next_line_help=true, help_heading=LOGGING_HELP_HEADING)]
    pub save_solver_input_file: Option<PathBuf>,

    /// Use the experimental optimized / dirty-clean rewriter, instead of the default rewriter
    #[arg(long, default_value_t = false, global = true, help_heading = EXPERIMENTAL_HELP_HEADING)]
    pub use_optimised_rewriter: bool,

    /// Exit after all comprehensions have been unrolled, printing the number of expressions at that point.
    ///
    /// This is only compatible with the default rewriter.
    ///
    /// This flag is useful to compare how comprehension optimisations, such as expand-ac, effect
    /// rewriting.
    #[arg(long, default_value_t = false, global = true, help_heading = DEBUG_HELP_HEADING)]
    pub exit_after_unrolling: bool,
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

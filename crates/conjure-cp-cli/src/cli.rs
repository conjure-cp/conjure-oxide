use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use clap_complete::Shell;
use conjure_cp::settings::{Parser as InputParser, QuantifiedExpander, Rewriter, SolverFamily};

use crate::{pretty, solve, test_solve};

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
    Pretty(pretty::Args),
    // Run the language server
    ServerLSP,
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

    /// Log verbosely
    #[arg(long, short = 'v', help = "Log verbosely to stderr", global = true, help_heading = LOGGING_HELP_HEADING)]
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

    /// Which parser to use.
    ///
    /// Possible values: `tree-sitter`, `via-conjure`.
    #[arg(
        long,
        default_value_t = InputParser::TreeSitter,
        value_parser = parse_parser,
        global = true,
        help_heading = EXPERIMENTAL_HELP_HEADING
    )]
    pub parser: InputParser,

    /// Which rewriter to use.
    ///
    /// Possible values: `naive`, `morph`.
    #[arg(long, default_value_t = Rewriter::Naive, value_parser = parse_rewriter, global = true, help_heading = EXPERIMENTAL_HELP_HEADING)]
    pub rewriter: Rewriter,

    /// Which strategy to use for expanding quantified variables in comprehensions.
    ///
    /// Possible values: `native`, `via-solver`, `via-solver-ac`.
    #[arg(long, default_value_t = QuantifiedExpander::Native, value_parser = parse_quantified_expander, global = true, help_heading = OPTIMISATIONS_HELP_HEADING)]
    pub quantified_expander: QuantifiedExpander,

    /// Solver family to use.
    ///
    /// Possible values: `minion`, `sat`, `sat-log`, `sat-direct`, `sat-order`;
    /// with `smt` feature: `smt` and `smt-<ints>-<matrices>[-nodiscrete]`
    /// where `<ints>` is `lia` or `bv`, and `<matrices>` is `arrays` or `atomic`.
    #[arg(
        long,
        value_name = "SOLVER",
        value_parser = parse_solver_family,
        default_value = "minion",
        short = 's',
        global = true
    )]
    pub solver: SolverFamily,

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

    /// Use the experimental rewrite cache for the dirty-clean rewriter
    #[arg(long, default_value_t = false, global = true, help_heading = EXPERIMENTAL_HELP_HEADING)]
    pub use_cache: bool,

    /// Use treemorph on naive mode. TODO
    #[arg(long, default_value_t = false, global = true, help_heading = EXPERIMENTAL_HELP_HEADING)]
    pub use_naive: bool,

    /// Exit after all comprehensions have been unrolled, printing the number of expressions at that point.
    ///
    /// This is only compatible with the default rewriter.
    ///
    /// This flag is useful to compare how comprehension optimisations, such as expand-ac, effect
    /// rewriting.
    #[arg(long, default_value_t = false, global = true, help_heading = DEBUG_HELP_HEADING)]
    pub exit_after_unrolling: bool,

    /// Stop the solver after the given timeout.
    ///
    /// Currently only SMT supports this feature.
    #[arg(long, global = true, help_heading = OPTIMISATIONS_HELP_HEADING)]
    pub solver_timeout: Option<humantime::Duration>,

    /// Generate log files
    #[arg(long, default_value_t = false, global = true, help_heading = LOGGING_HELP_HEADING)]
    pub log: bool,

    /// Output file for conjure-oxide's text logs
    #[arg(long, value_name = "LOGFILE", global = true, help_heading = LOGGING_HELP_HEADING)]
    pub logfile: Option<PathBuf>,

    /// Output file for conjure-oxide's json logs
    #[arg(long, value_name = "JSON LOGFILE", global = true, help_heading = LOGGING_HELP_HEADING)]
    pub logfile_json: Option<PathBuf>,
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

fn parse_quantified_expander(input: &str) -> Result<QuantifiedExpander, String> {
    input.parse()
}

fn parse_rewriter(input: &str) -> Result<Rewriter, String> {
    input.parse()
}

fn parse_solver_family(input: &str) -> Result<SolverFamily, String> {
    input.parse()
}

fn parse_parser(input: &str) -> Result<InputParser, String> {
    input.parse()
}

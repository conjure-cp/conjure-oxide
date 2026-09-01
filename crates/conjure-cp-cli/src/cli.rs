use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};

use clap_complete::Shell;
use conjure_cp::settings::{
    Channelling, DEFAULT_HEURISTIC_SEED, DEFAULT_MINION_DISCRETE_THRESHOLD, Heuristic,
    Parser as InputParser, QuantifiedExpander, Rewriter, SolverFamily,
};
use conjure_cp::solver::adaptors::{MinionValueOrder, MinionVariableOrder};

use crate::{pretty, solve, test_solve};

pub(crate) const LOGGING_HELP_HEADING: Option<&str> = Some("Logging & Output");
pub(crate) const CONFIGURATION_HELP_HEADING: Option<&str> = Some("Configuration");

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
    before_help = "Full documentation can be found online at: https://conjure-cp.github.io/conjure-oxide",
    // Free `-h` for `--heuristic`; help remains available as `--help`.
    disable_help_flag = true
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
    /// Print help
    #[arg(long, action = clap::ArgAction::Help, global = true)]
    pub help: (),

    /// Extra rule sets to enable
    #[arg(long, value_name = "EXTRA_RULE_SETS", global = true)]
    pub extra_rule_sets: Vec<String>,

    /// Increase stderr logging detail (-v: stages, -vv: rule applications, -vvv: rule attempts).
    ///
    /// Rule-attempt logging can be expensive and produce a very large amount of output.
    #[arg(
        long,
        short = 'v',
        action = ArgAction::Count,
        global = true,
        conflicts_with = "quiet",
        help_heading = LOGGING_HELP_HEADING
    )]
    pub verbose: u8,

    /// Disable warning and progress logs on stderr
    #[arg(long, short = 'q', global = true, help_heading = LOGGING_HELP_HEADING)]
    pub quiet: bool,

    /// Output file for the default rule trace.
    #[arg(long, global = true, help_heading=LOGGING_HELP_HEADING)]
    pub rule_trace: Option<PathBuf>,

    /// Output file for aggregated rule-application counts.
    ///
    /// The file is updated incrementally in the format:
    /// `total_rule_applications: N`, followed by one line per rule.
    #[arg(long, global = true, help_heading=LOGGING_HELP_HEADING)]
    pub rule_trace_aggregates: Option<PathBuf>,

    /// Continue rule trace generation during solver-time CDP rewrites.
    ///
    /// This is off by default, so follow-up dominance-blocking rewrites do not contribute to the
    /// trace.
    #[arg(long, default_value_t = false, global = true, help_heading=LOGGING_HELP_HEADING)]
    pub rule_trace_cdp: bool,

    /// Output file for the rule-attempt trace in CSV format.
    ///
    /// Each row includes: elapsed_s, rule_level, rule_name, rule_set, status, expression.
    #[arg(
        long = "rule-attempt-trace",
        global = true,
        help_heading=LOGGING_HELP_HEADING
    )]
    pub rule_attempt_trace: Option<PathBuf>,

    /// Which parser to use.
    ///
    /// Possible values: `tree-sitter`, `via-conjure`.
    #[arg(
        long,
        default_value_t = InputParser::default(),
        value_parser = parse_parser,
        global = true,
        help_heading = CONFIGURATION_HELP_HEADING
    )]
    pub parser: InputParser,

    /// Which rewriter to use.
    ///
    /// Possible values: `baseline`, `optimised`, `baseline+prefilter`, or `baseline+worklist`.
    ///
    /// Option meanings:
    /// - `prefilter`: skip rules whose declared expression kinds cannot match; strong win vs
    ///   baseline and part of `optimised`.
    /// - `worklist`: drive rewriting from persistent dirty queues instead of repeated full scans;
    ///   strong win vs baseline and part of `optimised`.
    #[arg(long, default_value_t = Rewriter::default(), value_parser = parse_rewriter, global = true, help_heading = CONFIGURATION_HELP_HEADING)]
    pub rewriter: Rewriter,

    /// Which strategy to use for expanding quantified variables in comprehensions.
    ///
    /// Possible values: `auto`, `native`, `via-solver`, `via-solver-ac`. `auto` chooses
    /// between native and solver-backed expansion from the comprehension's estimated size and
    /// available pruning constraints.
    #[arg(
        long,
        default_value_t = QuantifiedExpander::Auto,
        value_parser = parse_comprehension_expander,
        global = true,
        help_heading = CONFIGURATION_HELP_HEADING
    )]
    pub comprehension_expander: QuantifiedExpander,

    /// Heuristic for selecting an answer when multiple modelling choices are applicable.
    ///
    /// Possible values: `f` (first), `r` (random), `c` (compact), `i` (interactive). Compact
    /// minimises the representation-domain size for representation choices and the resulting AST
    /// depth for equally-applicable rewrite rules. Interactive prompts on stderr, or uses
    /// `--responses` when provided. `x` (all) is reserved for model generation and is not
    /// supported by the CLI yet.
    #[arg(
        long,
        short = 'h',
        default_value_t = Heuristic::Compact,
        value_parser = parse_cli_heuristic,
        global = true,
        help_heading = CONFIGURATION_HELP_HEADING
    )]
    pub heuristic: Heuristic,

    /// Comma-separated 1-based answers for the interactive heuristic (`-h i`).
    ///
    /// If provided, these are used as the answers during interactive model generation instead of
    /// prompting the user.
    #[arg(
        long,
        value_name = "INTS",
        value_delimiter = ',',
        global = true,
        help_heading = CONFIGURATION_HELP_HEADING
    )]
    pub responses: Vec<usize>,

    /// Seed used by the random heuristic.
    #[arg(
        long,
        default_value_t = DEFAULT_HEURISTIC_SEED,
        global = true,
        help_heading = CONFIGURATION_HELP_HEADING
    )]
    pub seed: u64,

    /// Seed used by the backend solver's random search behaviour.
    #[arg(
        long,
        default_value_t = 0,
        global = true,
        help_heading = CONFIGURATION_HELP_HEADING
    )]
    pub solver_seed: u32,

    /// Whether multiple representations of the same declaration may be channelled together.
    ///
    /// Possible values: `no`, `yes`. Channelling is disabled by default. Enable `yes` to allow
    /// different representations of the same variable at different call sites, e.g.
    /// `1 in (x :: set (representation packed) of int) /\ 2 in (x :: set (representation occurrence) of int)`.
    #[arg(
        long,
        default_value_t = Channelling::No,
        value_parser = parse_cli_channelling,
        global = true,
        help_heading = CONFIGURATION_HELP_HEADING
    )]
    pub channelling: Channelling,

    /// Solver to use.
    ///
    /// Possible values: `minion`, `sat`, `z3`.
    ///
    /// How a model is expressed for the chosen solver -- which SAT encoding an integer gets, or
    /// which Z3 theory -- is a modelling choice made per declaration, not part of the solver name.
    /// Use `--heuristic` to steer those choices and `--channelling` to allow more than one per
    /// declaration.
    #[arg(
        long,
        value_name = "SOLVER",
        value_parser = parse_solver_family,
        default_value = "minion",
        short = 's',
        global = true,
        help_heading = CONFIGURATION_HELP_HEADING
    )]
    pub solver: SolverFamily,

    /// Int-domain size threshold for using Minion `DISCRETE` variables.
    ///
    /// If an int domain has size <= this value, Conjure Oxide emits `DISCRETE`; otherwise `BOUND`.
    #[arg(
        long,
        default_value_t = DEFAULT_MINION_DISCRETE_THRESHOLD,
        global = true,
        help_heading = CONFIGURATION_HELP_HEADING
    )]
    pub minion_discrete_threshold: usize,

    /// Override Minion variable ordering.
    ///
    /// Possible values: `static`, `sdf`, `srf`, `ldf`, `random`, `conflict`, `wdeg`,
    /// `domoverwdeg`.
    #[arg(
        long,
        value_name = "ORDER",
        value_parser = parse_minion_variable_order,
        global = true,
        help_heading = CONFIGURATION_HELP_HEADING
    )]
    pub minion_varorder: Option<MinionVariableOrder>,

    /// Override Minion value ordering.
    ///
    /// Possible values: `ascend`, `descend`, `random`.
    #[arg(
        long,
        value_name = "ORDER",
        value_parser = parse_minion_value_order,
        global = true,
        help_heading = CONFIGURATION_HELP_HEADING
    )]
    pub minion_valorder: Option<MinionValueOrder>,

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

    /// Stop the solver after the given timeout.
    ///
    /// Minion has one-second timeout resolution, so finer durations are rounded up.
    #[arg(long, global = true, help_heading = CONFIGURATION_HELP_HEADING)]
    pub solver_timeout: Option<humantime::Duration>,

    /// Write general logs to this file
    #[arg(long, value_name = "PATH", global = true, help_heading = LOGGING_HELP_HEADING)]
    pub log_file: Option<PathBuf>,

    /// Format used by --log-file [default: text]
    #[arg(long, value_enum, requires = "log_file", global = true, help_heading = LOGGING_HELP_HEADING)]
    pub log_format: Option<LogFormat>,

    /// Detail written by --log-file [default: stages]
    #[arg(long, value_enum, requires = "log_file", global = true, help_heading = LOGGING_HELP_HEADING)]
    pub log_detail: Option<LogDetail>,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum LogFormat {
    #[default]
    Text,
    Json,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum LogDetail {
    #[default]
    Stages,
    Applications,
    Attempts,
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

fn parse_comprehension_expander(input: &str) -> Result<QuantifiedExpander, String> {
    input.parse()
}

fn parse_cli_heuristic(input: &str) -> Result<Heuristic, String> {
    match input.parse::<Heuristic>()? {
        Heuristic::All => {
            Err("heuristic 'x' (all) is not supported by the command line yet".to_string())
        }
        heuristic => Ok(heuristic),
    }
}

fn parse_cli_channelling(input: &str) -> Result<Channelling, String> {
    input.parse::<Channelling>()
}

fn parse_rewriter(input: &str) -> Result<Rewriter, String> {
    input.parse::<Rewriter>()
}

fn parse_solver_family(input: &str) -> Result<SolverFamily, String> {
    input.parse()
}

fn parse_parser(input: &str) -> Result<InputParser, String> {
    input.parse()
}

fn parse_minion_value_order(input: &str) -> Result<MinionValueOrder, String> {
    match input {
        "ascend" => Ok(MinionValueOrder::Ascend),
        "descend" => Ok(MinionValueOrder::Descend),
        "random" => Ok(MinionValueOrder::Random),
        other => Err(format!(
            "unknown minion value order '{other}', expected one of: ascend, descend, random"
        )),
    }
}

fn parse_minion_variable_order(input: &str) -> Result<MinionVariableOrder, String> {
    match input {
        "static" => Ok(MinionVariableOrder::Static),
        "sdf" => Ok(MinionVariableOrder::SmallestDomainFirst),
        "srf" => Ok(MinionVariableOrder::SmallestRatioFirst),
        "ldf" => Ok(MinionVariableOrder::LargestDomainFirst),
        "random" => Ok(MinionVariableOrder::Random),
        "conflict" => Ok(MinionVariableOrder::Conflict),
        "wdeg" => Ok(MinionVariableOrder::WeightedDegree),
        "domoverwdeg" => Ok(MinionVariableOrder::DomainOverWeightedDegree),
        other => Err(format!(
            "unknown minion variable order '{other}', expected one of: static, sdf, srf, ldf, \
             random, conflict, wdeg, domoverwdeg"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_is_the_default_cli_heuristic() {
        let cli = Cli::try_parse_from(["conjure-oxide", "solve", "model.essence"]).unwrap();
        assert_eq!(cli.global_args.heuristic, Heuristic::Compact);
    }

    #[test]
    fn auto_is_the_default_comprehension_expander() {
        let cli = Cli::try_parse_from(["conjure-oxide", "solve", "model.essence"]).unwrap();
        assert_eq!(
            cli.global_args.comprehension_expander,
            QuantifiedExpander::Auto
        );
    }

    #[test]
    fn solver_seed_defaults_to_zero_and_can_be_overridden() {
        let cli = Cli::try_parse_from(["conjure-oxide", "solve", "model.essence"]).unwrap();
        assert_eq!(cli.global_args.solver_seed, 0);

        let cli = Cli::try_parse_from([
            "conjure-oxide",
            "solve",
            "model.essence",
            "--solver-seed",
            "42",
        ])
        .unwrap();
        assert_eq!(cli.global_args.solver_seed, 42);
    }

    #[test]
    fn parses_all_minion_variable_orders() {
        let cases = [
            ("static", MinionVariableOrder::Static),
            ("sdf", MinionVariableOrder::SmallestDomainFirst),
            ("srf", MinionVariableOrder::SmallestRatioFirst),
            ("ldf", MinionVariableOrder::LargestDomainFirst),
            ("random", MinionVariableOrder::Random),
            ("conflict", MinionVariableOrder::Conflict),
            ("wdeg", MinionVariableOrder::WeightedDegree),
            ("domoverwdeg", MinionVariableOrder::DomainOverWeightedDegree),
        ];

        for (name, expected) in cases {
            let cli = Cli::try_parse_from([
                "conjure-oxide",
                "solve",
                "model.essence",
                "--minion-varorder",
                name,
            ])
            .unwrap();
            assert_eq!(cli.global_args.minion_varorder, Some(expected));
        }
    }
}

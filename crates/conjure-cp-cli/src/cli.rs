use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};

use clap_complete::Shell;
use conjure_cp::settings::{
    Channelling, DEFAULT_HEURISTIC_SEED, Heuristic, Parser as InputParser, QuantifiedExpander,
    Rewriter, SolverFamily,
};
use conjure_cp::solver::adaptors::{MinionValueOrder, MinionVariableOrder};
use git_version::git_version;

use crate::{pretty, solve, test_solve};

pub(crate) const LOGGING_HELP_HEADING: Option<&str> = Some("Logging & Output");
pub(crate) const CONFIGURATION_HELP_HEADING: Option<&str> = Some("Configuration");
pub(crate) const MODELLING_HELP_HEADING: Option<&str> = Some("Modelling choices");

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
    disable_help_flag = true,
    version = git_version!(),
    disable_version_flag = true,
    display_name = "conjure-oxide",
    // clap's derive turns this on for a required subcommand; keep the concise
    // "requires a subcommand" error instead of dumping the full help.
    arg_required_else_help = false
)]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: Command,

    #[command(flatten)]
    pub global_args: GlobalArgs,

    /// Print version
    // `ArgAction::Version` is handled by clap while parsing, so `--version` works on its own,
    // without the otherwise-required subcommand.
    #[arg(long = "version", short = 'V', action = ArgAction::Version)]
    pub version: (),
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
    /// Counts are kept in memory and written when the program exits, in the format:
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
        help_heading = MODELLING_HELP_HEADING
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
        help_heading = MODELLING_HELP_HEADING
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
        help_heading = MODELLING_HELP_HEADING
    )]
    pub responses: Vec<usize>,

    /// Seed used by the random heuristic.
    #[arg(
        long,
        default_value_t = DEFAULT_HEURISTIC_SEED,
        global = true,
        help_heading = MODELLING_HELP_HEADING
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
    /// Possible values: `no`, `yes`, `uniform`. Channelling is disabled by default.
    /// `uniform` uses one representation kind per type family throughout the model. Enable `yes` for
    /// different representations of the same variable at different call sites, e.g.
    /// `1 in (x :: set (representation packed) of int) /\ 2 in (x :: set (representation occurrence) of int)`.
    #[arg(
        long,
        default_value_t = Channelling::No,
        value_parser = parse_cli_channelling,
        global = true,
        help_heading = MODELLING_HELP_HEADING
    )]
    pub channelling: Channelling,

    /// Pin the SAT element composition: implication or support.
    #[arg(long = "sat-encoding-element", global = true, help_heading = MODELLING_HELP_HEADING)]
    pub element_encoding: Option<conjure_cp::ast::sat_decision::ElementEncoding>,

    /// Pin the SAT table composition: tuple, mdd or binary-support (constant two-column relations).
    #[arg(long = "sat-encoding-table", global = true, help_heading = MODELLING_HELP_HEADING)]
    pub table_encoding: Option<conjure_cp::ast::sat_decision::TableEncoding>,

    /// Pin allDifferent: pairwise, or value-amo (requires Direct or Boolean value indicators).
    #[arg(long = "sat-encoding-alldifferent", global = true, help_heading = MODELLING_HELP_HEADING)]
    pub alldifferent_encoding: Option<conjure_cp::ast::sat_decision::AllDifferentEncoding>,

    /// Pin the SAT AMO encoder: pairwise, ladder, bitwise, commander, bimander, two-product,
    /// pindakaas-pairwise, pindakaas-ladder or pindakaas-bitwise.
    /// If omitted, compact chooses per constraint; portfolio heuristics share one family choice.
    #[arg(long = "sat-encoding-amo", global = true, help_heading = MODELLING_HELP_HEADING)]
    pub amo_encoding: Option<conjure_cp::ast::sat_decision::AmoEncoding>,

    /// Pin the SAT cardinality encoder: rustsat-totalizer or pindakaas-sorting-network.
    #[arg(long = "sat-encoding-cardinality", global = true, help_heading = MODELLING_HELP_HEADING)]
    pub cardinality_encoding: Option<conjure_cp::ast::sat_decision::CardinalityEncoding>,

    /// Pin the SAT weighted encoder: rustsat-generalized-totalizer, rustsat-binary-adder, pindakaas-bdd, rustsat-dynamic-poly-watchdog, or pindakaas-swc.
    #[arg(long = "sat-encoding-pb", global = true, help_heading = MODELLING_HELP_HEADING)]
    pub pb_encoding: Option<conjure_cp::ast::sat_decision::PbEncoding>,

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

    /// Pin the int-domain span threshold for using Minion `DISCRETE` variables.
    ///
    /// If maximum - minimum + 1 <= this value, emit `DISCRETE`; otherwise `BOUND`.
    /// Constraints requiring `DISCRETE` override this choice. If omitted, the heuristic chooses
    /// between 10, zero (all `BOUND`), and unlimited (all `DISCRETE`). Compact chooses 10.
    #[arg(
        long,
        global = true,
        help_heading = MODELLING_HELP_HEADING
    )]
    pub minion_discrete_threshold: Option<usize>,

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

    /// Stop the solver after the given cumulative wall-clock timeout.
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
    let family = input.parse()?;
    if family == SolverFamily::Z3 && !cfg!(feature = "z3") {
        return Err("Z3 solver support was not compiled in (enable the `z3` feature).".into());
    }
    Ok(family)
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

    #[cfg(not(feature = "z3"))]
    #[test]
    fn selecting_z3_without_support_reports_the_missing_feature() {
        let error =
            Cli::try_parse_from(["conjure-oxide", "solve", "model.essence", "--solver", "z3"])
                .unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
        assert!(
            error
                .to_string()
                .contains("Z3 solver support was not compiled in")
        );
    }

    /// Regression test for #1631: `--version` used to fail as it requires a subcommand.
    #[test]
    fn version_flag_works_without_a_subcommand() {
        for flag in ["--version", "-V"] {
            let err = Cli::try_parse_from(["conjure-oxide", flag]).unwrap_err();
            assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
            assert!(err.to_string().starts_with("conjure-oxide "));
        }
    }

    #[test]
    fn compact_is_the_default_cli_heuristic() {
        let cli = Cli::try_parse_from(["conjure-oxide", "solve", "model.essence"]).unwrap();
        assert_eq!(cli.global_args.heuristic, Heuristic::Compact);
    }

    #[test]
    fn modelling_choices_are_unpinned_unless_supplied() {
        let cli = Cli::try_parse_from(["conjure-oxide", "solve", "model.essence"]).unwrap();
        let args = cli.global_args;
        assert_eq!(args.minion_discrete_threshold, None);
        assert_eq!(args.amo_encoding, None);
        assert_eq!(args.cardinality_encoding, None);
        assert_eq!(args.pb_encoding, None);
        assert_eq!(args.element_encoding, None);
        assert_eq!(args.table_encoding, None);
        assert_eq!(args.alldifferent_encoding, None);

        let cli = Cli::try_parse_from([
            "conjure-oxide",
            "solve",
            "model.essence",
            "--heuristic",
            "r",
            "--minion-discrete-threshold",
            "0",
            "--sat-encoding-amo",
            "ladder",
            "--sat-encoding-cardinality",
            "rustsat-totalizer",
            "--sat-encoding-pb",
            "pindakaas-bdd",
            "--sat-encoding-element",
            "support",
            "--sat-encoding-table",
            "mdd",
            "--sat-encoding-alldifferent",
            "pairwise",
        ])
        .unwrap();
        let args = cli.global_args;
        assert_eq!(args.minion_discrete_threshold, Some(0));
        assert_eq!(
            args.amo_encoding,
            Some(conjure_cp::ast::sat_decision::AmoEncoding::Ladder)
        );
        assert!(args.cardinality_encoding.is_some());
        assert!(args.pb_encoding.is_some());
        assert!(args.element_encoding.is_some());
        assert!(args.table_encoding.is_some());
        assert!(args.alldifferent_encoding.is_some());
    }

    #[test]
    fn modelling_choices_have_their_own_help_group() {
        use clap::CommandFactory;
        let command = Cli::command();
        for name in [
            "amo_encoding",
            "cardinality_encoding",
            "pb_encoding",
            "element_encoding",
            "table_encoding",
            "alldifferent_encoding",
            "minion_discrete_threshold",
            "channelling",
            "comprehension_expander",
            "heuristic",
            "responses",
            "seed",
        ] {
            let arg = command
                .get_arguments()
                .find(|arg| arg.get_id().as_str() == name)
                .unwrap();
            assert_eq!(arg.get_help_heading(), MODELLING_HELP_HEADING);
        }
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

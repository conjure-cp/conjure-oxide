//! conjure_oxide solve sub-command
#![allow(clippy::unwrap_used)]
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;
use std::{
    fs::File,
    path::{Path, PathBuf},
};

use clap::ValueHint;
use conjure_cp::Model;
use conjure_cp::ast::SatEncodingDecision;
use conjure_cp::instantiate::{instantiate_model, validate_instantiation_conditions};
use conjure_cp::pipeline::Pipeline;
use conjure_cp::solutions::SolutionOptions;
use conjure_cp_cli::stats::{
    RecordedRunConfig, RecordedRunStats, StatsFormat, replace_config_run_stats,
};
use conjure_cp_cli::utils::conjure::{solutions_to_essence, solutions_to_json};
use conjure_cp_cli::utils::rule_trace::RuleApplicationCounter;
use conjure_cp_cli::utils::simplified_json::{
    domains_from_model, param_model_from_assignments, params_from_simplified_json_str,
    solution_to_simplified_json, solutions_to_simplified_json_string,
};

use crate::cli::{GlobalArgs, LOGGING_HELP_HEADING};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    /// Conjure-compatible Essence multi-solution / per-solution files
    #[default]
    Essence,
    /// Conjure `--output-format=json` simplified JSON
    Json,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberOfSolutions {
    All,
    Limit(i32),
}

impl NumberOfSolutions {
    fn as_solver_limit(self) -> i32 {
        match self {
            NumberOfSolutions::All => 0,
            NumberOfSolutions::Limit(limit) => limit,
        }
    }
}

fn parse_number_of_solutions(input: &str) -> Result<NumberOfSolutions, String> {
    if input.eq_ignore_ascii_case("all") {
        return Ok(NumberOfSolutions::All);
    }

    let limit = input
        .parse::<i32>()
        .map_err(|_| "expected a positive integer or 'all'".to_string())?;

    if limit <= 0 {
        return Err("expected a positive integer or 'all'".to_string());
    }

    Ok(NumberOfSolutions::Limit(limit))
}

#[derive(Clone, Debug, clap::Args)]
pub struct Args {
    /// The input Essence problem file
    #[arg(value_name = "INPUT_ESSENCE", value_hint = ValueHint::FilePath)]
    pub essence_file: PathBuf,

    /// The input Essence parameter file
    #[arg(value_name = "PARAM_ESSENCE", value_hint = ValueHint::FilePath)]
    pub param_file: Option<PathBuf>,

    /// Write execution statistics; the .toml or .json extension selects the format.
    #[arg(long, value_name = "PATH", value_hint = ValueHint::FilePath,
        value_parser = parse_stats_path, help_heading = LOGGING_HELP_HEADING)]
    pub stats: Option<PathBuf>,

    /// Do not run the solver.
    ///
    /// The rewritten model is printed to stdout in an Essence-style syntax
    /// (but is not necessarily valid Essence).
    #[arg(long, default_value_t = false)]
    pub no_run_solver: bool,

    /// Number of solutions to return. Use a positive integer, or `all`.
    #[arg(
        long,
        short = 'n',
        default_value = "1",
        value_name = "N|all",
        value_parser = parse_number_of_solutions
    )]
    pub number_of_solutions: NumberOfSolutions,

    /// Format for printed / saved solutions
    #[arg(long, value_enum, default_value_t = OutputFormat::Essence, help_heading=LOGGING_HELP_HEADING)]
    pub output_format: OutputFormat,

    /// Write all solutions into a single file (default: true).
    ///
    /// Pass `--solutions-in-one-file=false` to emit one file (or stdout record) per solution.
    #[arg(
        long,
        default_value_t = true,
        action = clap::ArgAction::Set,
        value_name = "BOOL",
        help_heading = LOGGING_HELP_HEADING
    )]
    pub solutions_in_one_file: bool,

    /// Save solutions to the given path (Essence `.solutions` / JSON `.solutions.json`, or a
    /// filename stem when `--solutions-in-one-file=false`)
    #[arg(long, short = 'o', value_hint = ValueHint::FilePath,help_heading=LOGGING_HELP_HEADING)]
    pub output: Option<PathBuf>,

    /// When optimising, retain every improving solution Minion reports instead of only the last
    /// one.
    #[arg(long, default_value_t = false)]
    pub keep_intermediate_solutions: bool,
}

fn parse_stats_path(input: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(input);
    StatsFormat::from_path(&path).map_err(|err| err.to_string())?;
    Ok(path)
}

pub fn run_solve_command(
    global_args: GlobalArgs,
    solve_args: Args,
    rule_application_counts: &RuleApplicationCounter,
) -> anyhow::Result<()> {
    let pipeline = Pipeline::new(global_args.library_config())?;
    pipeline
        .with_settings(|| run_solve(&pipeline, global_args, solve_args, rule_application_counts))
}

fn run_solve(
    pipeline: &Pipeline,
    global_args: GlobalArgs,
    solve_args: Args,
    rule_application_counts: &RuleApplicationCounter,
) -> anyhow::Result<()> {
    let essence_file = solve_args.essence_file.clone();
    let param_file = solve_args.param_file.clone();

    let context = pipeline.context();
    {
        let mut context = context.write().unwrap();
        context.essence_file_name = Some(essence_file.to_string_lossy().into_owned());
        context.param_file_name = param_file
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned());
    }

    let translation_started_at = Instant::now();
    let unified_model = pipeline.with_settings(|| -> anyhow::Result<Model> {
        let problem_model = pipeline.parse_file(&essence_file)?;
        match param_file {
            Some(param_file) => {
                let param_model = parse_param(pipeline, &param_file, &problem_model)?;
                Ok(instantiate_model(problem_model, param_model)?)
            }
            None => {
                let mut problem_model = problem_model;
                validate_instantiation_conditions(&mut problem_model)?;
                Ok(problem_model)
            }
        }
    })?;
    let rewritten_model = pipeline.rewrite(unified_model)?;

    // Capture representations and SAT encoding choices
    let mut model_choices = RecordedRunStats::default();

    record_model_choices(&rewritten_model, &mut model_choices);

    let translation_time = translation_started_at.elapsed().as_secs_f64();

    let solver_started_at = Instant::now();

    let measured = if solve_args.no_run_solver {
        println!("{}", rewritten_model);

        if let Some(ref path) = global_args.save_solver_input_file {
            pipeline.with_settings(|| -> anyhow::Result<()> {
                let solver = pipeline.solver()?.load_model(rewritten_model)?;
                let mut file: Box<dyn std::io::Write> = Box::new(File::create(path)?);
                solver.write_solver_input_file(&mut file)?;
                Ok(())
            })?;
        }

        None
    } else {
        Some(run_solver(
            pipeline,
            &global_args,
            &solve_args,
            rewritten_model,
        )?)
    };

    let solve_time = measured
        .as_ref()
        .map(|_| solver_started_at.elapsed().as_secs_f64());

    let backend_time = measured.as_ref().map(|t| t.0);
    let search_time = measured.as_ref().map(|t| t.1);
    let solution_processing_time = measured.as_ref().map(|t| t.2);

    // Still write translation statistics when the solver was not run.
    if let Some(ref path) = solve_args.stats {
        let config = RecordedRunConfig {
            parser: global_args.parser.to_string(),
            rewriter: global_args.rewriter.to_string(),
            comprehension_expander: global_args.comprehension_expander.to_string(),
            heuristic: global_args.heuristic.to_string(),
            channelling: global_args.channelling.to_string(),
            seed: global_args.seed,
            solver_seed: global_args.solver_seed,
            solver: global_args.solver.as_str(),
        };

        let case_name = solve_args
            .essence_file
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "input".to_owned());

        let mut row = RecordedRunStats {
            parser: config.parser.clone(),
            rewriter: config.rewriter.clone(),
            comprehension_expander: config.comprehension_expander.clone(),
            heuristic: config.heuristic.clone(),
            channelling: config.channelling.clone(),
            seed: config.seed,
            solver_seed: config.solver_seed,
            solver: config.solver.clone(),
            minion_discrete_threshold: global_args.minion_discrete_threshold,

            case_name,
            model_index: 0,
            status: "ok".to_owned(),

            solution_limit: (!solve_args.no_run_solver)
                .then(|| solve_args.number_of_solutions.as_solver_limit()),

            translation_time: Some(translation_time),
            solve_time,
            backend_time,
            search_time,
            solution_processing_time,

            ..Default::default()
        };

        row.representations = model_choices.representations;

        row.sat_encoding_amo = model_choices.sat_encoding_amo;
        row.sat_encoding_cardinality = model_choices.sat_encoding_cardinality;
        row.sat_encoding_pb = model_choices.sat_encoding_pb;
        row.sat_encoding_table = model_choices.sat_encoding_table;
        row.sat_encoding_element = model_choices.sat_encoding_element;
        row.sat_encoding_alldifferent = model_choices.sat_encoding_alldifferent;

        // Read the same rewrite-attempt counters used by exhaustive testing.
        let context_json = serde_json::to_value(context.read().unwrap().clone())?;

        let attempt_counts: Vec<u64> = context_json
            .get("stats")
            .and_then(|stats| stats.get("rewriterRuns"))
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|run| {
                run.get("rewriterRuleApplicationAttempts")
                    .and_then(serde_json::Value::as_u64)
            })
            .collect();

        if !attempt_counts.is_empty() {
            row.rule_trace.total_rule_attempts = Some(attempt_counts.iter().sum());
        }

        let counts = rule_application_counts.snapshot();
        row.rule_trace.total_rule_applications = Some(counts.values().copied().sum());
        row.rule_trace.rules = counts;

        // Write the completed statistics record only after all fields are populated.
        replace_config_run_stats(path, &config, vec![row])?;
    }
    Ok(())
}
fn record_model_choices(model: &Model, row: &mut RecordedRunStats) {
    row.representations = model
        .symbols()
        .iter_local()
        .flat_map(|(_, decl)| {
            decl.reprs()
                .iter()
                .map(|(name, _)| name.to_owned())
                .collect::<Vec<_>>()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut families: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    macro_rules! record {
        ($family:expr, $selection:expr) => {{
            $selection.iter().for_each(|selection| {
                families
                    .entry($family)
                    .or_default()
                    .insert(selection.algorithm.to_string());
            });
        }};
    }
    for decision in model.sat_decisions() {
        match decision {
            SatEncodingDecision::AtMostOne { encoding, .. } => {
                record!("amo", encoding);
            }
            SatEncodingDecision::Cardinality { encoding, .. } => {
                record!("cardinality", encoding);
            }
            SatEncodingDecision::CountRelation {
                encoding,
                amo_encoding,
                ..
            } => {
                record!("cardinality", encoding);
                record!("amo", amo_encoding);
            }
            SatEncodingDecision::PseudoBoolean { encoding, .. }
            | SatEncodingDecision::IntegerRelation { encoding, .. }
            | SatEncodingDecision::Objective { encoding, .. } => record!("pb", encoding),
            SatEncodingDecision::AllDifferent {
                encoding,
                amo_encoding,
                pb_encoding,
                ..
            } => {
                record!("alldifferent", encoding);
                record!("amo", amo_encoding);
                record!("pb", pb_encoding);
            }
            SatEncodingDecision::Table {
                encoding,
                pb_encoding,
                ..
            } => {
                record!("table", encoding);
                record!("pb", pb_encoding);
            }
            SatEncodingDecision::Element {
                encoding,
                pb_encoding,
                ..
            } => {
                record!("element", encoding);
                record!("pb", pb_encoding);
            }
            _ => {}
        }
    }
    row.sat_encoding_amo = families
        .remove("amo")
        .unwrap_or_default()
        .into_iter()
        .collect();
    row.sat_encoding_cardinality = families
        .remove("cardinality")
        .unwrap_or_default()
        .into_iter()
        .collect();
    row.sat_encoding_pb = families
        .remove("pb")
        .unwrap_or_default()
        .into_iter()
        .collect();
    row.sat_encoding_table = families
        .remove("table")
        .unwrap_or_default()
        .into_iter()
        .collect();
    row.sat_encoding_element = families
        .remove("element")
        .unwrap_or_default()
        .into_iter()
        .collect();
    row.sat_encoding_alldifferent = families
        .remove("alldifferent")
        .unwrap_or_default()
        .into_iter()
        .collect();
}
/// Parse an Essence or simplified-JSON parameter file.
pub(crate) fn parse_param(
    pipeline: &Pipeline,
    file_path: &Path,
    problem_model: &Model,
) -> anyhow::Result<Model> {
    if file_path
        .extension()
        .is_some_and(|extension| extension == "json")
    {
        tracing::info!(target: "conjure::stage", path = %file_path.display(), "parsing parameter file");
        let text = std::fs::read_to_string(file_path)?;
        let given_domains = domains_from_model(problem_model);
        let params = params_from_simplified_json_str(&text, &given_domains)?;
        return Ok(param_model_from_assignments(
            params,
            &given_domains,
            pipeline.context(),
        ));
    }
    pipeline.parse_file(file_path)
}

fn run_solver(
    pipeline: &Pipeline,
    global_args: &GlobalArgs,
    cmd_args: &Args,
    model: Model,
) -> anyhow::Result<(f64, f64, f64)> {
    let (solutions, measured) = pipeline.solve_with_timings(
        model,
        &SolutionOptions {
            number_of_solutions: cmd_args.number_of_solutions.as_solver_limit(),
            keep_intermediate_solutions: cmd_args.keep_intermediate_solutions,
            solver_input_file: global_args.save_solver_input_file.clone(),
            rule_trace_cdp: global_args.rule_trace_cdp,
        },
    )?;

    let output_started_at = Instant::now();

    tracing::debug!(
        solutions = %solutions_to_json(&solutions),
        "solver solutions"
    );

    write_solutions(&solutions, cmd_args)?;

    Ok((
        measured.backend_time_s,
        measured.search_time_s,
        measured.solution_processing_time_s + output_started_at.elapsed().as_secs_f64(),
    ))
}

fn write_solutions(
    solutions: &[std::collections::BTreeMap<conjure_cp::ast::Name, conjure_cp::ast::Literal>],
    cmd_args: &Args,
) -> anyhow::Result<()> {
    if cmd_args.solutions_in_one_file {
        let body = match cmd_args.output_format {
            OutputFormat::Essence => solutions_to_essence(solutions),
            OutputFormat::Json => solutions_to_simplified_json_string(solutions)?,
        };
        match &cmd_args.output {
            None => print!("{body}"),
            Some(path) => {
                std::fs::write(path, body.as_bytes())?;
                println!("Solutions saved to {:?}", path.canonicalize()?);
            }
        }
        return Ok(());
    }

    // One file / record per solution.
    for (index, solution) in solutions.iter().enumerate() {
        let body = match cmd_args.output_format {
            OutputFormat::Essence => solutions_to_essence(std::slice::from_ref(solution)),
            OutputFormat::Json => {
                let value = solution_to_simplified_json(solution)?;
                format!("{}\n", serde_json::to_string_pretty(&value)?)
            }
        };
        match &cmd_args.output {
            None => print!("{body}"),
            Some(path) => {
                let file_path = per_solution_output_path(path, index + 1, cmd_args.output_format);
                std::fs::write(&file_path, body.as_bytes())?;
                println!("Solution saved to {:?}", file_path.canonicalize()?);
            }
        }
    }
    Ok(())
}

fn per_solution_output_path(base: &Path, index: usize, format: OutputFormat) -> PathBuf {
    let stem = base
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("solution");
    let parent = base.parent().unwrap_or_else(|| std::path::Path::new("."));
    let extension = match format {
        OutputFormat::Essence => "solution",
        OutputFormat::Json => "solution.json",
    };
    parent.join(format!("{stem}-{index:06}.{extension}"))
}

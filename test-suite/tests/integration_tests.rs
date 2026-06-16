#![allow(clippy::expect_used)]
use git_version as _;

use conjure_cp::defaults::DEFAULT_RULE_SETS;
use conjure_cp::parse::tree_sitter::parse_essence_file_native;
use conjure_cp::rule_engine::{rewrite_morph, rewrite_naive};
use conjure_cp::solver::Solver;
use conjure_cp::solver::adaptors::*;
use conjure_cp_cli::utils::testing::{normalize_solutions_for_comparison, read_default_rule_trace};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::fs::File;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tracing_subscriber::{Layer, filter::EnvFilter, filter::FilterFn, fmt, layer::SubscriberExt};

use std::sync::Arc;
use std::sync::RwLock;

use conjure_cp::ast::{Literal, Name};
use conjure_cp::context::Context;
use conjure_cp::parse::tree_sitter::parse_essence_file;
use conjure_cp::rule_engine::resolve_rule_sets;
use conjure_cp::settings::{
    Parser, QuantifiedExpander, Rewriter, SolverFamily, set_comprehension_expander,
    set_current_parser, set_current_rewriter, set_current_solver_family,
    set_default_rule_trace_enabled, set_minion_discrete_threshold,
    set_rule_trace_aggregates_enabled, set_rule_trace_enabled, set_rule_trace_verbose_enabled,
};
use conjure_cp_cli::utils::conjure::solutions_to_json;
use conjure_cp_cli::utils::conjure::{get_solutions, get_solutions_from_conjure_with_stats};
use conjure_cp_cli::utils::testing::save_stats_json;
use conjure_cp_cli::utils::testing::{read_solutions_json, save_solutions_json};
#[allow(clippy::single_component_path_imports, unused_imports)]
use conjure_cp_rules;
use pretty_assertions::assert_eq;
use test_suite::AcceptMode;
use test_suite::TestConfig;
use test_suite::golden_files::assert_no_redundant_expected_files;
use test_suite::test_config::{
    RecordedRunStats, round_expected_time, upsert_expected_time_config,
    upsert_recorded_run_stats_config, upsert_status_config,
};

#[derive(Clone, Copy, Debug)]
struct RunCase<'a> {
    parser: Parser,
    rewriter: Rewriter,
    comprehension_expander: QuantifiedExpander,
    solver: SolverFamily,
    case_name: &'a str,
}

#[derive(Clone, Copy, Debug, Default)]
struct RunTimings {
    translation_time_s: f64,
    solve_time_s: f64,
}

impl RunTimings {
    fn add(&mut self, other: Self) {
        self.translation_time_s += other.translation_time_s;
        self.solve_time_s += other.solve_time_s;
    }
}

fn run_integration_test_with_timeout<F>(
    test_name: &str,
    test_dir: &str,
    run_test: F,
) -> Result<(), Box<dyn Error>>
where
    F: FnOnce() -> Result<(), Box<dyn Error>>,
{
    let Some(timeout) = test_case_timeout()? else {
        return run_test();
    };

    if std::env::var_os("CONJURE_OXIDE_TEST_TIMEOUT_CHILD").is_some() {
        return run_test();
    }

    let mut child = Command::new(std::env::current_exe()?)
        .arg(test_name)
        .arg("--exact")
        .arg("--nocapture")
        .env("CONJURE_OXIDE_TEST_TIMEOUT_CHILD", "1")
        .stdin(Stdio::null())
        .spawn()?;
    let started_at = Instant::now();

    loop {
        if let Some(status) = child.try_wait()? {
            if status.success() {
                return Ok(());
            }

            return Err(format!("timed child test {test_name} failed with {status}").into());
        }

        if started_at.elapsed() >= timeout {
            child.kill()?;
            let _ = child.wait();
            if AcceptMode::from_env().accepts_outputs() {
                upsert_status_config(
                    &Path::new(test_dir).join("config.toml"),
                    &format!("timeout({})", timeout.as_secs()),
                )?;
            }
            return Err(format!(
                "test {test_name} exceeded TEST_CASE_TIMEOUT={}s",
                timeout.as_secs()
            )
            .into());
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

fn test_case_timeout() -> Result<Option<Duration>, Box<dyn Error>> {
    match std::env::var("TEST_CASE_TIMEOUT") {
        Ok(value) => {
            let seconds = value.parse::<u64>().map_err(|err| {
                format!("invalid TEST_CASE_TIMEOUT value '{value}', expected seconds: {err}")
            })?;
            Ok(Some(Duration::from_secs(seconds)))
        }
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(err) => Err(Box::new(err)),
    }
}

fn run_case_label(
    path: &str,
    essence_base: &str,
    extension: &str,
    run_case: RunCase<'_>,
) -> String {
    format!(
        "test_dir={path}, model={essence_base}.{extension}, parser={}, rewriter={}, comprehension_expander={}, solver={}",
        run_case.parser,
        run_case.rewriter,
        run_case.comprehension_expander,
        run_case.solver.as_str()
    )
}

fn integration_test(path: &str, essence_base: &str, extension: &str) -> Result<(), Box<dyn Error>> {
    let result = integration_test_inner_with_status(path, essence_base, extension);

    if AcceptMode::from_env().accepts_outputs() {
        let status = if result.is_ok() { "ok" } else { "fail" };
        upsert_status_config(&Path::new(path).join("config.toml"), status)?;
    }

    result
}

fn integration_test_inner_with_status(
    path: &str,
    essence_base: &str,
    extension: &str,
) -> Result<(), Box<dyn Error>> {
    let accept_mode = AcceptMode::from_env();
    let accept = accept_mode.accepts_outputs();
    let started_at = Instant::now();

    if accept {
        clean_test_dir_for_accept(path, essence_base, extension)?;
    }

    let file_config: TestConfig =
        if let Ok(config_contents) = fs::read_to_string(format!("{path}/config.toml")) {
            toml::from_str(&config_contents).unwrap()
        } else {
            Default::default()
        };

    let config = file_config;

    let validate_with_conjure = config.validate_with_conjure;
    let minion_discrete_threshold = config.minion_discrete_threshold;
    let number_of_solutions = config.number_of_solutions.as_solver_limit();

    let parsers = config
        .configured_parsers()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
    let rewriters = config
        .configured_rewriters()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
    let comprehension_expanders = config
        .configured_comprehension_expanders()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
    let solvers = config
        .configured_solvers()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;

    // Conjure output depends only on the input model, so cache it once per test case.
    let model_path = format!("{path}/{essence_base}.{extension}");
    let conjure_solutions = if accept && validate_with_conjure {
        eprintln!("[integration] loading Conjure reference solutions for {model_path}");
        let conjure_run = get_solutions_from_conjure_with_stats(
            &model_path,
            None,
            Default::default(),
            number_of_solutions,
        )
        .map_err(|err| {
            std::io::Error::other(format!(
                "failed to fetch Conjure reference solutions for {model_path}: {err}"
            ))
        })?;

        Some((Arc::new(conjure_run.solutions), conjure_run.timings))
    } else {
        if accept && !validate_with_conjure {
            eprintln!("[integration] skipping Conjure validation for {model_path}");
        }
        None
    };
    let conjure_solution_values = conjure_solutions
        .as_ref()
        .map(|(solutions, _)| Arc::clone(solutions));
    let conjure_timings = conjure_solutions.and_then(|(_, timings)| timings);
    let mut allowed_expected_files = BTreeSet::new();
    let mut oxide_timings = RunTimings::default();

    for parser in parsers.iter().copied() {
        for rewriter in rewriters.clone() {
            for comprehension_expander in comprehension_expanders.clone() {
                for solver in solvers.clone() {
                    let case_name = run_case_name(parser, rewriter, comprehension_expander);
                    let run_case = RunCase {
                        parser,
                        rewriter,
                        comprehension_expander,
                        solver,
                        case_name: case_name.as_str(),
                    };
                    let file = File::create(format!(
                        "{path}/{}-{}-generated-rule-trace.txt",
                        run_case.case_name,
                        run_case.solver.as_str()
                    ))?;
                    let subscriber = Arc::new(
                        tracing_subscriber::registry().with(
                            fmt::layer()
                                .with_writer(file)
                                .with_level(false)
                                .without_time()
                                .with_target(false)
                                .with_filter(EnvFilter::new("rule_engine_rule_trace=trace"))
                                .with_filter(FilterFn::new(|meta| {
                                    meta.target() == "rule_engine_rule_trace"
                                })),
                        ),
                    )
                        as Arc<dyn tracing::Subscriber + Send + Sync>;
                    let run_label = run_case_label(path, essence_base, extension, run_case);
                    eprintln!("[integration] running {run_label}");
                    let default_rule_trace_enabled = matches!(rewriter, Rewriter::Naive);
                    set_rule_trace_enabled(true);
                    set_default_rule_trace_enabled(default_rule_trace_enabled);
                    set_rule_trace_verbose_enabled(false);
                    set_rule_trace_aggregates_enabled(false);
                    let run_timings = tracing::subscriber::with_default(subscriber, || {
                        integration_test_inner(
                            path,
                            essence_base,
                            extension,
                            run_case,
                            minion_discrete_threshold,
                            number_of_solutions,
                            conjure_solution_values.clone(),
                            accept,
                        )
                    })
                    .map_err(|err| std::io::Error::other(format!("{run_label}: {err}")))?;
                    oxide_timings.add(run_timings);
                    allowed_expected_files.extend(expected_integration_files_for_case(
                        run_case.case_name,
                        solver,
                    ));
                }
            }
        }
    }

    assert_no_redundant_expected_files(Path::new(path), &allowed_expected_files, None)?;

    if accept_mode.records_expected_time() {
        let observed_expected_time = round_expected_time(started_at.elapsed());
        let config_path = Path::new(path).join("config.toml");
        if let Some(expected_time) =
            accept_mode.expected_time_to_record(config.expected_time, observed_expected_time)
        {
            upsert_expected_time_config(&config_path, expected_time)?;
        }
    }

    if accept && validate_with_conjure {
        if let Some(conjure_timings) = conjure_timings {
            let config_path = Path::new(path).join("config.toml");
            upsert_recorded_run_stats_config(
                &config_path,
                RecordedRunStats {
                    oxide_translation_time: oxide_timings.translation_time_s,
                    oxide_solve_time: oxide_timings.solve_time_s,
                    conjure_translation_time: conjure_timings.translation_time_s,
                    conjure_driver_translation_time: conjure_timings.conjure_translation_time_s,
                    savilerow_translation_time: conjure_timings.savilerow_translation_time_s,
                    conjure_solve_time: conjure_timings.solve_time_s,
                },
            )?;
        }
    }

    Ok(())
}

/// Runs an integration test for a given Conjure model by:
/// 1. Parsing the model from an Essence file.
/// 2. Rewriting the model according to predefined rule sets.
/// 3. Solving the model using the Minion solver and validating the solutions.
/// 4. Comparing generated rule traces with expected outputs.
///
/// This function operates in multiple stages:
///
/// - **Parsing Stage**
///   - **Stage 1a (Default)**: Reads the Essence model file and verifies that it parses correctly using `parser = "tree-sitter"`.
///   - **Stage 1b (Optional)**: Reads the Essence model file and verifies that it parses correctly using `parser = "via-conjure"`.
///
/// - **Rewrite Stage**
///   - **Stage 2a**: Applies a set of rules to the parsed model and validates the result.
///
/// - **Solution Stage**
///   - **Stage 3a (Default)**: Uses Minion to solve the model and save the solutions.
///   - **Stage 3b (ACCEPT only)**: Compares solutions against Conjure-generated solutions.
///
/// - **Rule Trace Validation Stage**
///   - **Stage 4a (Default)**: Checks that the generated rules match expected traces.
///
/// # Arguments
///
/// * `path` - The file path where the Essence model and other resources are located.
/// * `essence_base` - The base name of the Essence model file.
/// * `extension` - The file extension for the Essence model.
///
/// # Errors
///
/// Returns an error if any stage fails due to a mismatch with expected results or file I/O issues.
#[allow(clippy::unwrap_used)]
fn integration_test_inner(
    path: &str,
    essence_base: &str,
    extension: &str,
    run_case: RunCase<'_>,
    minion_discrete_threshold: usize,
    number_of_solutions: i32,
    conjure_solutions: Option<Arc<Vec<BTreeMap<Name, Literal>>>>,
    accept: bool,
) -> Result<RunTimings, Box<dyn Error>> {
    let parser = run_case.parser;
    let rewriter = run_case.rewriter;
    let comprehension_expander = run_case.comprehension_expander;
    let solver_fam = run_case.solver;
    let case_name = run_case.case_name;

    let context: Arc<RwLock<Context<'static>>> = Default::default();

    set_current_parser(parser);
    set_current_rewriter(rewriter);
    set_comprehension_expander(comprehension_expander);
    set_current_solver_family(solver_fam);
    set_minion_discrete_threshold(minion_discrete_threshold);

    // File path
    let file_path = format!("{path}/{essence_base}.{extension}");

    let translation_started_at = Instant::now();

    // Stage 1a/1b: Parse the model using the selected parser.
    let parsed_model = match parser {
        Parser::TreeSitter => {
            let mut ctx = context.as_ref().write().unwrap();
            ctx.essence_file_name = Some(format!("{path}/{essence_base}.{extension}"));
            parse_essence_file_native(&file_path, context.clone())?
        }
        Parser::ViaConjure => parse_essence_file(&file_path, context.clone())?,
    };
    // Stage 2a: Rewrite the model using the rule engine
    let mut extra_rules = vec![];

    if let SolverFamily::Sat(sat_encoding) = solver_fam {
        extra_rules.push(sat_encoding.as_rule_set());
    }

    let mut rules_to_load = DEFAULT_RULE_SETS.to_vec();
    rules_to_load.extend(extra_rules);

    let rule_sets = resolve_rule_sets(solver_fam, &rules_to_load)?;

    let model = parsed_model;

    let rewritten_model = match rewriter {
        Rewriter::Naive => rewrite_naive(&model, &rule_sets, false)?,
        Rewriter::Morph(config) => rewrite_morph(model, &rule_sets, false, config),
    };
    let translation_time_s = translation_started_at.elapsed().as_secs_f64();

    let solver_input_file = None;
    let solver = match solver_fam {
        SolverFamily::Minion => Solver::new(Minion::default()),
        SolverFamily::Sat(_) => Solver::new(Sat::default()),

        SolverFamily::Smt(_) => Solver::new(Smt::default()),
    };

    let solver_started_at = Instant::now();
    let solutions = {
        let solved = get_solutions(
            solver,
            rewritten_model,
            number_of_solutions,
            &solver_input_file,
            false,
        )?;
        save_solutions_json(&solved, path, case_name, solver_fam)?;
        solved
    };
    let solve_time_s = solver_started_at.elapsed().as_secs_f64();

    // Stage 3b: Check solutions against Conjure when accept mode is enabled and validation is enabled.
    if accept && conjure_solutions.is_some() {
        let conjure_solutions = conjure_solutions
            .as_deref()
            .expect("conjure solutions should be present when Conjure validation is enabled");

        let username_solutions = normalize_solutions_for_comparison(&solutions);
        let conjure_solutions = normalize_solutions_for_comparison(conjure_solutions);

        let mut conjure_solutions_json = solutions_to_json(&conjure_solutions);
        let mut username_solutions_json = solutions_to_json(&username_solutions);

        conjure_solutions_json.sort_all_objects();
        username_solutions_json.sort_all_objects();

        assert_eq!(
            username_solutions_json, conjure_solutions_json,
            "Solutions (<) do not match conjure (>)!"
        );
    }

    // When accept mode is enabled, copy all generated files to expected
    if accept {
        // Always overwrite these ones. Unlike the rest, we don't need to selectively do these
        // based on the test results, so they don't get done later.
        copy_generated_to_expected(path, case_name, "solutions", "json", solver_fam)?;
        copy_human_trace_generated_to_expected(path, case_name, solver_fam)?;
    }

    // Check Stage 3a (solutions)
    let expected_solutions_json = read_solutions_json(path, case_name, "expected", solver_fam)?;
    let username_solutions_json = solutions_to_json(&solutions);
    assert_eq!(username_solutions_json, expected_solutions_json);

    // TODO: Implement rule trace validation for morph
    match rewriter {
        Rewriter::Morph(_) => {}
        Rewriter::Naive => {
            let generated = read_default_rule_trace(path, case_name, "generated", &solver_fam)?;
            let expected = read_default_rule_trace(path, case_name, "expected", &solver_fam)?;

            assert_eq!(
                expected, generated,
                "Generated rule trace does not match the expected trace!"
            );
        }
    }

    save_stats_json(context, path, case_name, solver_fam)?;

    Ok(RunTimings {
        translation_time_s,
        solve_time_s,
    })
}

fn run_case_name(
    parser: Parser,
    rewriter: Rewriter,
    comprehension_expander: QuantifiedExpander,
) -> String {
    format!("{parser}-{rewriter}-{comprehension_expander}")
}

/// Returns the expected snapshot files for an executed integration run case.
fn expected_integration_files_for_case(case_name: &str, solver: SolverFamily) -> BTreeSet<String> {
    let solver_name = solver.as_str();
    BTreeSet::from([
        format!("{case_name}-{solver_name}.expected-solutions.json"),
        format!("{case_name}-{solver_name}-expected-rule-trace.txt"),
    ])
}

fn clean_test_dir_for_accept(
    path: &str,
    essence_base: &str,
    extension: &str,
) -> Result<(), std::io::Error> {
    let input_filename = format!("{essence_base}.{extension}");

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let entry_path = entry.path();

        if file_name == input_filename || file_name == "config.toml" {
            continue;
        }

        if entry_path.is_dir() {
            std::fs::remove_dir_all(entry_path)?;
        } else {
            std::fs::remove_file(entry_path)?;
        }
    }

    Ok(())
}

fn copy_human_trace_generated_to_expected(
    path: &str,
    test_name: &str,
    solver: SolverFamily,
) -> Result<(), std::io::Error> {
    let solver_name = solver.as_str();

    std::fs::copy(
        format!("{path}/{test_name}-{solver_name}-generated-rule-trace.txt"),
        format!("{path}/{test_name}-{solver_name}-expected-rule-trace.txt"),
    )?;
    Ok(())
}

fn copy_generated_to_expected(
    path: &str,
    test_name: &str,
    stage: &str,
    extension: &str,
    solver: SolverFamily,
) -> Result<(), std::io::Error> {
    let marker = solver.as_str();

    std::fs::copy(
        format!("{path}/{test_name}-{marker}.generated-{stage}.{extension}"),
        format!("{path}/{test_name}-{marker}.expected-{stage}.{extension}"),
    )?;
    Ok(())
}

#[test]
fn assert_conjure_present() {
    conjure_cp_cli::find_conjure::conjure_executable().unwrap();
}

include!(concat!(env!("OUT_DIR"), "/gen_tests.rs"));

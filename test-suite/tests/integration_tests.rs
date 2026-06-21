#![allow(clippy::expect_used)]
use git_version as _;

use conjure_cp::defaults::DEFAULT_RULE_SETS;
use conjure_cp::parse::tree_sitter::parse_essence_file_native;
use conjure_cp::rule_engine::{rewrite_morph, rewrite_naive};
use conjure_cp::solver::Solver;
use conjure_cp::solver::adaptors::*;
use conjure_cp_cli::utils::testing::{
    DEFAULT_TEXT_SNAPSHOT_CHARACTER_LIMIT, normalize_solutions_for_comparison,
    read_default_rule_trace, truncate_to_first_chars,
};
use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tracing_subscriber::{Layer, filter::EnvFilter, filter::FilterFn, fmt, layer::SubscriberExt};

#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::sync::Arc;
use std::sync::RwLock;

use conjure_cp::ast::{Literal, Model, Name};
use conjure_cp::context::Context;
use conjure_cp::instantiate::instantiate_model;
use conjure_cp::parse::tree_sitter::parse_essence_file;
use conjure_cp::rule_engine::resolve_rule_sets;
use conjure_cp::settings::{
    Parser, QuantifiedExpander, Rewriter, SolverFamily, set_comprehension_expander,
    set_current_parser, set_current_rewriter, set_current_solver_family,
    set_default_rule_trace_enabled, set_minion_discrete_threshold,
    set_rule_trace_aggregates_enabled, set_rule_trace_enabled, set_rule_trace_verbose_enabled,
};
use conjure_cp_cli::utils::conjure::solutions_to_json;
use conjure_cp_cli::utils::conjure::{
    ConjureSolveCaptureOptions, get_solutions, get_solutions_from_conjure_with_stats,
};
use conjure_cp_cli::utils::testing::save_stats_json;
use conjure_cp_cli::utils::testing::{read_solutions_json, save_solutions_json};
#[allow(clippy::single_component_path_imports, unused_imports)]
use conjure_cp_rules;
use pretty_assertions::assert_eq;
use test_suite::AcceptMode;
use test_suite::TestConfig;
use test_suite::diagnostics::{
    DIAGNOSTICS_DIR, FailureRecord, clear_diagnostics, conjure_artifacts_dir, copy_file_if_exists,
    oxide_artifacts_dir, write_failure_record, write_oxide_failure_text,
};
use test_suite::golden_files::assert_no_redundant_expected_files;
use test_suite::test_config::{
    RecordedRunStats, RuleTraceAggregateStats, read_stats_or_default, round_expected_time,
    stats_path, upsert_expected_time_stats, upsert_recorded_run_stats,
    upsert_rule_trace_aggregate_stats, upsert_status_stats, upsert_tool_status_stats,
};
use test_suite::text_files::write_text_with_trailing_newline;

const DISABLE_TRACING_ENV: &str = "CONJURE_OXIDE_TEST_DISABLE_TRACING";

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

    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg(test_name)
        .arg("--exact")
        .env("CONJURE_OXIDE_TEST_TIMEOUT_CHILD", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    pass_integration_test_env(&mut command);

    #[cfg(unix)]
    command.process_group(0);

    let mut child = command.spawn()?;
    let stdout = child_output_reader(child.stdout.take());
    let stderr = child_output_reader(child.stderr.take());
    let started_at = Instant::now();

    loop {
        if let Some(status) = child.try_wait()? {
            if status.success() {
                return Ok(());
            }

            let child_output = format_child_output(stdout, stderr);
            return Err(
                format!("timed child test {test_name} failed with {status}{child_output}").into(),
            );
        }

        if started_at.elapsed() >= timeout {
            terminate_timed_child(&mut child)?;
            let child_output = format_child_output(stdout, stderr);
            upsert_status_stats(
                &stats_path(Path::new(test_dir)),
                &format!("timeout({})", timeout.as_secs()),
            )?;
            let _ = write_failure_record(
                Path::new(test_dir),
                &FailureRecord {
                    stage: "timeout".to_string(),
                    message: format!(
                        "test {test_name} exceeded TEST_CASE_TIMEOUT={}s{child_output}",
                        timeout.as_secs()
                    ),
                    run_label: None,
                },
            );
            return Err(format!(
                "test {test_name} exceeded TEST_CASE_TIMEOUT={}s{child_output}",
                timeout.as_secs(),
            )
            .into());
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Starts a background reader for a piped child-process stream.
fn child_output_reader(
    output: Option<impl Read + Send + 'static>,
) -> Option<std::thread::JoinHandle<Vec<u8>>> {
    output.map(|mut output| {
        std::thread::spawn(move || {
            let mut buffer = Vec::new();
            let _ = output.read_to_end(&mut buffer);
            buffer
        })
    })
}

/// Formats captured child-process output for failure reports.
fn format_child_output(
    stdout: Option<std::thread::JoinHandle<Vec<u8>>>,
    stderr: Option<std::thread::JoinHandle<Vec<u8>>>,
) -> String {
    let stdout = join_child_output(stdout);
    let stderr = join_child_output(stderr);
    let stdout = cleaned_child_output(&stdout);
    let stderr = cleaned_child_output(&stderr);

    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (false, true) => format!("\n\nchild stdout:\n{stdout}"),
        (true, false) => format!("\n\nchild stderr:\n{stderr}"),
        (false, false) => format!("\n\nchild stdout:\n{stdout}\n\nchild stderr:\n{stderr}"),
    }
}

/// Collects bytes read by a child-output reader thread.
fn join_child_output(handle: Option<std::thread::JoinHandle<Vec<u8>>>) -> Vec<u8> {
    handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default()
}

/// Removes noisy libtest status lines from captured child-process output.
fn cleaned_child_output(output: &[u8]) -> String {
    String::from_utf8_lossy(output)
        .lines()
        .filter(|line| line.trim() != "running 1 test")
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

#[cfg(unix)]
fn terminate_timed_child(child: &mut Child) -> Result<(), Box<dyn Error>> {
    let process_group = format!("-{}", child.id());
    let _ = Command::new("kill")
        .arg("-TERM")
        .arg(&process_group)
        .status();

    std::thread::sleep(Duration::from_millis(500));
    if child.try_wait()?.is_none() {
        let _ = Command::new("kill")
            .arg("-KILL")
            .arg(&process_group)
            .status();
    }

    let _ = child.wait();
    Ok(())
}

#[cfg(not(unix))]
fn terminate_timed_child(child: &mut Child) -> Result<(), Box<dyn Error>> {
    child.kill()?;
    let _ = child.wait();
    Ok(())
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

fn pass_integration_test_env(command: &mut Command) {
    for key in [
        "TEST_CASE_TIMEOUT",
        "ACCEPT",
        "MAX_EXPECTED_TIME",
        DISABLE_TRACING_ENV,
    ] {
        if let Ok(value) = std::env::var(key) {
            command.env(key, value);
        }
    }
}

fn test_tracing_disabled() -> bool {
    match std::env::var(DISABLE_TRACING_ENV) {
        Ok(value) => !matches!(value.as_str(), "" | "0" | "false" | "False" | "FALSE"),
        Err(std::env::VarError::NotPresent) => false,
        Err(std::env::VarError::NotUnicode(_)) => true,
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

fn param_file_in_test_dir(path: &str) -> Option<String> {
    fs::read_dir(path).ok().and_then(|entries| {
        entries
            .filter_map(|entry| entry.ok())
            .find(|entry| entry.path().extension().is_some_and(|ext| ext == "param"))
            .map(|entry| entry.path().to_string_lossy().to_string())
    })
}

fn integration_test(path: &str, essence_base: &str, extension: &str) -> Result<(), Box<dyn Error>> {
    let result = integration_test_inner_with_status(path, essence_base, extension);

    if result.is_ok() {
        let _ = clear_diagnostics(Path::new(path));
    }

    if AcceptMode::from_env().accepts_outputs() {
        let status = if result.is_ok() { "ok" } else { "fail" };
        upsert_status_stats(&stats_path(Path::new(path)), status)?;
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

    let run_stats = read_stats_or_default(&stats_path(Path::new(path)))?;
    let config = file_config;

    let skip_conjure_validation = config.should_skip_conjure_validation();
    let minion_discrete_threshold = config.minion_discrete_threshold;
    let number_of_solutions = config.number_of_solutions.as_solver_limit();
    let keep_intermediate_solutions = config.keep_intermediate_solutions;

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
    let param_file = param_file_in_test_dir(path);
    let stats_path = stats_path(Path::new(path));
    let mut conjure_captured = false;
    let conjure_solutions = if accept && !skip_conjure_validation {
        let conjure_run = match get_solutions_from_conjure_with_stats(
            &model_path,
            param_file.as_deref(),
            Default::default(),
            number_of_solutions,
            ConjureSolveCaptureOptions {
                artifact_dir: Some(conjure_artifacts_dir(Path::new(path))),
                savilerow_options: Some("-O0".to_string()),
            },
        ) {
            Ok(conjure_run) => {
                conjure_captured = true;
                upsert_tool_status_stats(&stats_path, "conjure", "ok")?;
                conjure_run
            }
            Err(err) => {
                upsert_tool_status_stats(&stats_path, "conjure", "fail")?;
                record_integration_failure(
                    path,
                    FailureRecord {
                        stage: "conjure".to_string(),
                        message: err.to_string(),
                        run_label: None,
                    },
                    number_of_solutions,
                    false,
                );
                return Err(std::io::Error::other(format!(
                    "failed to fetch Conjure reference solutions for {model_path}: {err}"
                ))
                .into());
            }
        };

        Some((Arc::new(conjure_run.solutions), conjure_run.timings))
    } else {
        None
    };
    let conjure_solution_values = conjure_solutions
        .as_ref()
        .map(|(solutions, _)| Arc::clone(solutions));
    let conjure_timings = conjure_solutions.and_then(|(_, timings)| timings);
    let mut allowed_expected_files = BTreeSet::new();
    let mut oxide_timings = RunTimings::default();
    let rule_trace_snapshots_enabled = !test_tracing_disabled();

    let oxide_result = (|| -> Result<(), Box<dyn Error>> {
        for parser in parsers.iter().copied() {
            for rewriter in rewriters.clone() {
                for comprehension_expander in comprehension_expanders.clone() {
                    for solver in solvers.clone() {
                        let case_name = run_case_name(parser, comprehension_expander);
                        let run_case = RunCase {
                            parser,
                            rewriter,
                            comprehension_expander,
                            solver,
                            case_name: case_name.as_str(),
                        };
                        let run_label = run_case_label(path, essence_base, extension, run_case);
                        let default_rule_trace_enabled = matches!(rewriter, Rewriter::Rewrite(_));
                        set_rule_trace_enabled(rule_trace_snapshots_enabled);
                        set_default_rule_trace_enabled(
                            rule_trace_snapshots_enabled && default_rule_trace_enabled,
                        );
                        set_rule_trace_verbose_enabled(false);
                        set_rule_trace_aggregates_enabled(false);
                        let run_test = || {
                            integration_test_inner(
                                path,
                                essence_base,
                                extension,
                                run_case,
                                minion_discrete_threshold,
                                number_of_solutions,
                                keep_intermediate_solutions,
                                conjure_solution_values.clone(),
                                accept,
                                rule_trace_snapshots_enabled,
                            )
                        };
                        let run_timings = if rule_trace_snapshots_enabled {
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
                            tracing::subscriber::with_default(subscriber, run_test)
                        } else {
                            run_test()
                        }
                        .map_err(|err| {
                            let message = format!("{run_label}: {err}");
                            copy_oxide_run_artifacts(path, run_case, &message);
                            let _ = try_capture_oxide_minion(
                                path,
                                essence_base,
                                extension,
                                run_case,
                                minion_discrete_threshold,
                            );
                            std::io::Error::other(message)
                        })?;
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
        Ok(())
    })();

    if accept {
        let oxide_status = if oxide_result.is_ok() { "ok" } else { "fail" };
        upsert_tool_status_stats(&stats_path, "oxide", oxide_status)?;
    }

    if let Err(err) = &oxide_result {
        record_integration_failure(
            path,
            FailureRecord {
                stage: "oxide".to_string(),
                message: err.to_string(),
                run_label: None,
            },
            number_of_solutions,
            conjure_captured,
        );
    }

    oxide_result?;

    if accept_mode.records_expected_time() {
        let observed_expected_time = round_expected_time(started_at.elapsed());
        if let Some(expected_time) =
            accept_mode.expected_time_to_record(run_stats.expected_time, observed_expected_time)
        {
            upsert_expected_time_stats(&stats_path, expected_time)?;
        }
    }

    if accept && !skip_conjure_validation {
        if let Some(conjure_timings) = conjure_timings {
            upsert_recorded_run_stats(
                &stats_path,
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

    if accept && rule_trace_snapshots_enabled {
        let aggregates =
            collect_rule_trace_aggregates(Path::new(path), "-generated-rule-trace.txt")?;
        let aggregates = RuleTraceAggregateStats {
            total_rule_attempts: collect_rule_attempts(Path::new(path))?,
            ..aggregates
        };
        upsert_rule_trace_aggregate_stats(&stats_path, &aggregates)?;
    }

    Ok(())
}

/// Runs an integration test for a given Conjure model by:
/// 1. Parsing the model from an Essence file.
/// 2. Rewriting the model according to predefined rule sets.
/// 3. Solving the model using the Minion solver and validating the solutions.
/// 4. Comparing generated rule traces with expected outputs.
///
/// Set `CONJURE_OXIDE_TEST_DISABLE_TRACING=1` to skip rule trace generation and validation during
/// timing runs.
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
    keep_intermediate_solutions: bool,
    conjure_solutions: Option<Arc<Vec<BTreeMap<Name, Literal>>>>,
    accept: bool,
    rule_trace_snapshots_enabled: bool,
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

    let translation_started_at = Instant::now();

    // Stage 1a/1b: Parse the problem model and apply an optional param file.
    let parsed_model = parse_unified_problem_model(
        path,
        essence_base,
        extension,
        parser,
        param_file_in_test_dir(path).as_deref(),
        context.clone(),
    )?;
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
        Rewriter::Rewrite(config) => rewrite_naive(&model, &rule_sets, false, config)?,
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
            keep_intermediate_solutions,
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
        if rule_trace_snapshots_enabled {
            copy_human_trace_generated_to_expected(path, case_name, solver_fam)?;
        }
    }

    // Check Stage 3a (solutions)
    let expected_solutions_json = read_solutions_json(path, case_name, "expected", solver_fam)?;
    let username_solutions_json = solutions_to_json(&solutions);
    assert_eq!(username_solutions_json, expected_solutions_json);

    // TODO: Implement rule trace validation for morph
    if rule_trace_snapshots_enabled {
        match rewriter {
            Rewriter::Morph(_) => {}
            Rewriter::Rewrite(_) => {
                let generated = read_default_rule_trace(path, case_name, "generated", &solver_fam)?;
                let expected = read_default_rule_trace(path, case_name, "expected", &solver_fam)?;

                assert_eq!(
                    expected, generated,
                    "Generated rule trace does not match the expected trace!"
                );
            }
        }
    }

    save_stats_json(context, path, case_name, solver_fam)?;

    Ok(RunTimings {
        translation_time_s,
        solve_time_s,
    })
}

fn run_case_name(parser: Parser, comprehension_expander: QuantifiedExpander) -> String {
    format!("{parser}-{comprehension_expander}")
}

/// Returns the expected snapshot files for an executed integration run case.
fn expected_integration_files_for_case(case_name: &str, solver: SolverFamily) -> BTreeSet<String> {
    let solver_name = solver.as_str();
    BTreeSet::from([
        format!("{case_name}-{solver_name}.expected-solutions.json"),
        format!("{case_name}-{solver_name}-expected-rule-trace.txt"),
    ])
}

fn collect_rule_trace_aggregates(
    path: &Path,
    file_suffix: &str,
) -> Result<RuleTraceAggregateStats, std::io::Error> {
    let mut aggregates = RuleTraceAggregateStats::default();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if !file_name.ends_with(file_suffix) {
            continue;
        }

        let trace = fs::read_to_string(entry.path())?;
        for line in trace.lines() {
            let Some((_, rule_and_sets)) = line.split_once("~~>") else {
                continue;
            };
            let rule_name = rule_and_sets
                .trim()
                .split_once(' ')
                .map_or_else(|| rule_and_sets.trim(), |(rule, _)| rule);

            if rule_name.is_empty() {
                continue;
            }

            aggregates.total_rule_applications += 1;
            *aggregates.rules.entry(rule_name.to_string()).or_insert(0) += 1;
        }
    }

    Ok(aggregates)
}

fn collect_rule_attempts(path: &Path) -> Result<u64, std::io::Error> {
    let mut total = 0;

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if !file_name.ends_with("-stats.json") || file_name.contains("-naive-") {
            continue;
        }

        let stats: JsonValue = serde_json::from_str(&fs::read_to_string(entry.path())?)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        let Some(rewriter_runs) = stats
            .get("stats")
            .and_then(|stats| stats.get("rewriterRuns"))
            .and_then(JsonValue::as_array)
        else {
            continue;
        };

        total += rewriter_runs
            .iter()
            .filter_map(|run| {
                run.get("rewriterRuleApplicationAttempts")
                    .and_then(JsonValue::as_u64)
            })
            .sum::<u64>();
    }

    Ok(total)
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

        if file_name == input_filename
            || file_name == "config.toml"
            || file_name == "stats.toml"
            || file_name == DIAGNOSTICS_DIR
            || entry_path.extension().is_some_and(|ext| ext == "param")
        {
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

    let generated_path = format!("{path}/{test_name}-{solver_name}-generated-rule-trace.txt");
    let expected_path = format!("{path}/{test_name}-{solver_name}-expected-rule-trace.txt");
    let generated_trace = fs::read_to_string(generated_path)?;
    if generated_trace.chars().count() <= DEFAULT_TEXT_SNAPSHOT_CHARACTER_LIMIT {
        fs::write(expected_path, generated_trace)?;
        return Ok(());
    }

    let expected_trace =
        truncate_to_first_chars(&generated_trace, DEFAULT_TEXT_SNAPSHOT_CHARACTER_LIMIT);
    write_text_with_trailing_newline(Path::new(&expected_path), &expected_trace)?;
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

fn record_integration_failure(
    test_dir: &str,
    record: FailureRecord,
    number_of_solutions: i32,
    conjure_captured: bool,
) {
    let Some((essence_base, extension)) = essence_file_in_test_dir(test_dir) else {
        return;
    };
    let model_path = format!("{test_dir}/{essence_base}.{extension}");
    let param_file = param_file_in_test_dir(test_dir);
    let test_path = Path::new(test_dir);
    let _ = write_failure_record(test_path, &record);
    if !conjure_captured {
        let _ = capture_conjure_reference(
            test_path,
            &model_path,
            param_file.as_deref(),
            number_of_solutions,
        );
    }
}

fn capture_conjure_reference(
    test_dir: &Path,
    model_path: &str,
    param_file: Option<&str>,
    number_of_solutions: i32,
) -> Result<(), Box<dyn Error>> {
    let out_dir = conjure_artifacts_dir(test_dir);
    fs::create_dir_all(&out_dir)?;
    let context: Arc<RwLock<Context<'static>>> = Default::default();
    get_solutions_from_conjure_with_stats(
        model_path,
        param_file,
        context,
        number_of_solutions,
        ConjureSolveCaptureOptions {
            artifact_dir: Some(out_dir),
            savilerow_options: Some("-O0".to_string()),
        },
    )?;
    Ok(())
}

fn parse_unified_problem_model(
    path: &str,
    essence_base: &str,
    extension: &str,
    parser: Parser,
    param_file: Option<&str>,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Model, Box<dyn Error>> {
    let file_path = format!("{path}/{essence_base}.{extension}");

    {
        let mut ctx = context.as_ref().write().unwrap();
        ctx.essence_file_name = Some(file_path.clone());
        ctx.param_file_name = param_file.map(str::to_string);
    }

    let problem_model = match parser {
        Parser::TreeSitter => parse_essence_file_native(&file_path, context.clone())?,
        Parser::ViaConjure => parse_essence_file(&file_path, context.clone())?,
    };

    let Some(param_path) = param_file else {
        return Ok(problem_model);
    };

    let param_model = match parser {
        Parser::TreeSitter => parse_essence_file_native(param_path, context)?,
        Parser::ViaConjure => parse_essence_file(param_path, context)?,
    };

    Ok(instantiate_model(problem_model, param_model)?)
}

fn essence_file_in_test_dir(test_dir: &str) -> Option<(String, String)> {
    fs::read_dir(test_dir).ok().and_then(|entries| {
        entries.filter_map(Result::ok).find_map(|entry| {
            let path = entry.path();
            let extension = path.extension()?.to_str()?;
            if extension != "essence" {
                return None;
            }
            let name = path.file_name()?.to_str()?;
            if name.contains(".disabled") {
                return None;
            }
            Some((
                path.file_stem()?.to_str()?.to_string(),
                extension.to_string(),
            ))
        })
    })
}

fn copy_oxide_run_artifacts(path: &str, run_case: RunCase<'_>, message: &str) {
    let test_dir = Path::new(path);
    let _ = write_oxide_failure_text(test_dir, run_case.case_name, message);
    let solver = run_case.solver.as_str();
    let case_name = run_case.case_name;
    let oxide_dir = oxide_artifacts_dir(test_dir);
    let _ = copy_file_if_exists(
        &test_dir.join(format!("{case_name}-{solver}-generated-rule-trace.txt")),
        &oxide_dir.join(format!("{case_name}-{solver}-generated-rule-trace.txt")),
    );
    let _ = copy_file_if_exists(
        &test_dir.join(format!("{case_name}-{solver}.generated-solutions.json")),
        &oxide_dir.join(format!("{case_name}-{solver}.generated-solutions.json")),
    );
}

fn try_capture_oxide_minion(
    path: &str,
    essence_base: &str,
    extension: &str,
    run_case: RunCase<'_>,
    minion_discrete_threshold: usize,
) -> Result<(), Box<dyn Error>> {
    if !matches!(run_case.solver, SolverFamily::Minion) {
        return Ok(());
    }

    let context: Arc<RwLock<Context<'static>>> = Default::default();
    set_current_parser(run_case.parser);
    set_current_rewriter(run_case.rewriter);
    set_comprehension_expander(run_case.comprehension_expander);
    set_current_solver_family(run_case.solver);
    set_minion_discrete_threshold(minion_discrete_threshold);

    let parsed_model = parse_unified_problem_model(
        path,
        essence_base,
        extension,
        run_case.parser,
        param_file_in_test_dir(path).as_deref(),
        context.clone(),
    )?;

    let mut extra_rules = vec![];
    if let SolverFamily::Sat(sat_encoding) = run_case.solver {
        extra_rules.push(sat_encoding.as_rule_set());
    }
    let mut rules_to_load = DEFAULT_RULE_SETS.to_vec();
    rules_to_load.extend(extra_rules);
    let rule_sets = resolve_rule_sets(run_case.solver, &rules_to_load)?;

    let rewritten_model = match run_case.rewriter {
        Rewriter::Rewrite(config) => rewrite_naive(&parsed_model, &rule_sets, false, config)?,
        Rewriter::Morph(config) => rewrite_morph(parsed_model, &rule_sets, false, config),
    };

    let minion_path = oxide_artifacts_dir(Path::new(path)).join(format!(
        "{}-{}.minion",
        run_case.case_name,
        run_case.solver.as_str()
    ));
    if let Some(parent) = minion_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let solver = Solver::new(Minion::default()).load_model(rewritten_model)?;
    let mut file: Box<dyn std::io::Write> = Box::new(File::create(&minion_path)?);
    solver.write_solver_input_file(&mut file)?;
    Ok(())
}

#[test]
fn assert_conjure_present() {
    conjure_cp_cli::find_conjure::conjure_executable().unwrap();
}

include!(concat!(env!("OUT_DIR"), "/gen_tests.rs"));

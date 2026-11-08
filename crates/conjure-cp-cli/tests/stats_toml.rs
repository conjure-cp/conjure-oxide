//! Integration tests for the `conjure-oxide solve --stats-toml-path` CLI flag.
//!
//! Run with:
//! cargo test -p conjure-cp-cli --test stats_toml_cli -- --nocapture
//!
//! Each test starts a separate CLI process to avoid global rewrite/tracing state leaks.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "conjure-oxide-stats-test-{}-{nanos}-{id}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated test directory");
        Self(path)
    }

    fn file(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, contents).expect("write test fixture");
        path
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const MINION_MODEL: &str = "find x : int(1..3)\nsuch that x >= 2\n";

fn run_solve(
    dir: &TestDir,
    global_args: &[&str],
    model: &Path,
    stats_path: Option<&Path>,
) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_conjure-oxide"));
    cmd.current_dir(&dir.0).args(global_args).arg("solve");
    if let Some(path) = stats_path {
        cmd.arg("--stats-toml-path").arg(path);
    }
    cmd.arg(model).output().expect("launch conjure-oxide")
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "CLI exited with {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Parse the canonical TOML and return its single [[runs]] table.
fn read_single_run(path: &Path) -> toml::Value {
    let contents = fs::read_to_string(path).expect("statistics file was created");
    let parsed: toml::Table = contents.parse().expect("statistics file is valid TOML");
    let runs = parsed
        .get("runs")
        .and_then(toml::Value::as_array)
        .expect("canonical file contains [[runs]]");
    assert_eq!(runs.len(), 1, "one CLI solve should produce one run");
    runs[0].clone()
}

fn assert_nonnegative_finite_time(run: &toml::Value, field: &str) {
    let value = run
        .get(field)
        .and_then(toml::Value::as_float)
        .unwrap_or_else(|| panic!("missing or non-float timing field: {field}"));
    assert!(
        value.is_finite() && value >= 0.0,
        "{field} must be finite and nonnegative, got {value}"
    );
}

#[test]
fn minion_solve_writes_a_canonical_run_and_real_timings() {
    let dir = TestDir::new();
    let model = dir.file("minion.essence", MINION_MODEL);
    let stats = dir.path("stats.toml");

    let output = run_solve(&dir, &[], &model, Some(&stats));
    assert_success(&output);

    let run = read_single_run(&stats);
    assert_eq!(run.get("status").and_then(toml::Value::as_str), Some("ok"));
    assert_eq!(
        run.get("solver").and_then(toml::Value::as_str),
        Some("minion")
    );
    assert_eq!(
        run.get("case-name").and_then(toml::Value::as_str),
        Some("minion")
    );
    assert_eq!(
        run.get("solution-limit").and_then(toml::Value::as_integer),
        Some(1)
    );

    for field in [
        "translation-time",
        "solve-time",
        "backend-time",
        "search-time",
        "solution-processing-time",
    ] {
        assert_nonnegative_finite_time(&run, field);
    }
}

#[test]
fn minion_threshold_is_recorded_when_explicitly_selected() {
    let dir = TestDir::new();
    let model = dir.file("minion.essence", MINION_MODEL);
    let stats = dir.path("stats.toml");

    let output = run_solve(
        &dir,
        &["--solver", "minion", "--minion-discrete-threshold", "10"],
        &model,
        Some(&stats),
    );
    assert_success(&output);

    let run = read_single_run(&stats);
    assert_eq!(
        run.get("minion-discrete-threshold")
            .and_then(toml::Value::as_integer),
        Some(10)
    );
}

#[test]
fn rule_attempts_and_applications_are_recorded_consistently() {
    let dir = TestDir::new();
    let model = dir.file("rewrite.essence", MINION_MODEL);
    let stats = dir.path("stats.toml");

    let output = run_solve(&dir, &[], &model, Some(&stats));
    assert_success(&output);
    let run = read_single_run(&stats);

    let trace = run
        .get("rule-trace")
        .and_then(toml::Value::as_table)
        .expect("rule-trace table must exist");
    let attempts = trace
        .get("attempts")
        .and_then(toml::Value::as_integer)
        .expect("rule attempt count");
    let applications = trace
        .get("applications")
        .and_then(toml::Value::as_integer)
        .expect("rule application count");
    let rules = trace
        .get("rules")
        .and_then(toml::Value::as_table)
        .expect("per-rule application counts");

    assert!(attempts > 0, "this fixture should attempt rewrite rules");
    assert!(applications > 0, "this fixture should apply a rewrite rule");
    assert!(applications <= attempts);
    let sum: i64 = rules
        .values()
        .map(|count| count.as_integer().expect("rule count must be integer"))
        .sum();
    assert_eq!(
        sum, applications,
        "per-rule counts must sum to total applications"
    );
}

#[test]
fn omitting_stats_flag_keeps_normal_solution_output() {
    let dir = TestDir::new();
    let model = dir.file("model.essence", MINION_MODEL);
    let stats = dir.path("stats.toml");

    let without_stats = run_solve(&dir, &[], &model, None);
    assert_success(&without_stats);
    assert!(!stats.exists(), "no statistics file without the flag");

    let with_stats = run_solve(&dir, &[], &model, Some(&stats));
    assert_success(&with_stats);
    assert!(stats.exists());
    assert_eq!(
        without_stats.stdout, with_stats.stdout,
        "enabling TOML statistics must not change solution stdout"
    );
}

#[test]
fn invalid_statistics_output_path_reports_failure() {
    let dir = TestDir::new();
    let model = dir.file("model.essence", MINION_MODEL);
    let stats = dir.path("missing-parent/stats.toml");

    let output = run_solve(&dir, &[], &model, Some(&stats));
    assert!(
        !output.status.success(),
        "unwritable statistics path must fail"
    );
    assert!(!stats.exists());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("stats.toml")
            || stderr.contains("No such file")
            || stderr.contains("directory"),
        "the CLI should report an I/O error for the statistics path; got: {stderr}"
    );
}

#[test]
fn missing_input_fails_without_claiming_a_successful_run() {
    let dir = TestDir::new();
    let missing_model = dir.path("not-found.essence");
    let stats = dir.path("stats.toml");

    let output = run_solve(&dir, &[], &missing_model, Some(&stats));
    assert!(!output.status.success());
    assert!(
        !stats.exists(),
        "a missing input must not create a misleading status=ok record"
    );
}

#[test]
fn sat_run_writes_valid_encoding_metadata() {
    let dir = TestDir::new();
    let model = dir.file(
        "sat.essence",
        "find x : matrix indexed by [int(1..3)] of int(1..3)\nsuch that allDiff(x)\n",
    );
    let stats = dir.path("sat.toml");

    let output = run_solve(&dir, &["--solver", "sat"], &model, Some(&stats));
    assert_success(&output);
    let run = read_single_run(&stats);
    assert_eq!(run.get("solver").and_then(toml::Value::as_str), Some("sat"));

    for field in [
        "sat-encoding-amo",
        "sat-encoding-cardinality",
        "sat-encoding-pb",
        "sat-encoding-table",
        "sat-encoding-element",
        "sat-encoding-alldifferent",
    ] {
        let choices = run
            .get(field)
            .and_then(toml::Value::as_array)
            .unwrap_or_else(|| panic!("missing SAT encoding array: {field}"));
        assert!(
            choices.iter().all(|choice| choice.as_str().is_some()),
            "{field} must contain only encoding-name strings"
        );
    }
}

// This optional stronger assertion validates that the fixture actually triggers
// a SAT encoding decision. Enable after confirming the local SAT pipeline handles
// this allDiff model with an explicit choice.
#[test]

fn sat_all_different_records_a_selected_encoding() {
    let dir = TestDir::new();
    let model = dir.file(
        "sat-alldiff.essence",
        "find x : matrix indexed by [int(1..3)] of int(1..3)\nsuch that allDiff(x)\n",
    );
    let stats = dir.path("sat.toml");

    let output = run_solve(
        &dir,
        &["--solver", "sat", "--sat-encoding-alldifferent", "pairwise"],
        &model,
        Some(&stats),
    );
    assert_success(&output);
    let run = read_single_run(&stats);
    let selections = run
        .get("sat-encoding-alldifferent")
        .and_then(toml::Value::as_array)
        .expect("SAT all-different selections array");
    assert!(
        !selections.is_empty(),
        "the selected SAT encoding was not captured; inspect the model collection point"
    );
}

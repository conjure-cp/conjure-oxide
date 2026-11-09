//! Canonical TOML statistics shared by the CLI and exhaustive tests.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

fn quoted_toml_string(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

pub fn canonical_float(value: f64) -> String {
    let rounded = (value * 1_000_000.0).round() / 1_000_000.0;
    let mut rendered = format!("{rounded:.6}");
    while rendered.ends_with('0') {
        rendered.pop();
    }
    if rendered.ends_with('.') {
        rendered.push('0');
    }
    if rendered == "-0.0" {
        "0.0".to_string()
    } else {
        rendered
    }
}

fn canonical_rule_key(rule: &str) -> String {
    if !rule.is_empty()
        && rule
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        rule.to_string()
    } else {
        quoted_toml_string(rule)
    }
}

pub(super) fn deserialise_expected_time<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<u64>::deserialize(deserializer)
}

/// Identifies one configured exhaustive-test run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedRunConfig {
    pub parser: String,
    pub rewriter: String,
    pub comprehension_expander: String,
    pub heuristic: String,
    pub channelling: String,
    pub seed: u64,
    pub solver_seed: u32,
    pub solver: String,
}

fn rule_trace_rules_by_count_desc(
    rules: &std::collections::BTreeMap<String, u64>,
) -> Vec<(&String, u64)> {
    let mut sorted_rules: Vec<_> = rules.iter().map(|(rule, count)| (rule, *count)).collect();
    sorted_rules.sort_by(|(name_a, count_a), (name_b, count_b)| {
        count_b.cmp(count_a).then_with(|| name_a.cmp(name_b))
    });
    sorted_rules
}

/// Recorded exhaustive-run metadata for one test directory.
#[derive(Deserialize, Debug, Default)]
#[serde(default)]
#[serde(deny_unknown_fields)]
pub struct TestRunStats {
    /// Latest observed overall status, such as `ok`, `fail`, or `timeout(N)`.
    pub status: Option<String>,

    /// Coarse expected wall time bucket used by MAX_EXPECTED_TIME test selection.
    #[serde(
        default,
        rename = "expected-time",
        deserialize_with = "deserialise_expected_time"
    )]
    pub expected_time: Option<u64>,

    /// Metadata recorded for the Conjure plus Savile Row reference run.
    pub conjure: RecordedToolStats,

    /// One measured run per compiled model, including each expanded `x` choice.
    pub runs: Vec<RecordedRunStats>,
}

/// Flat timing and resolved modelling choices for one actual exhaustive model.
#[derive(Deserialize, Serialize, Debug, Default)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct RecordedRunStats {
    pub parser: String,
    pub rewriter: String,
    pub comprehension_expander: String,
    pub heuristic: String,
    pub channelling: String,
    pub seed: u64,
    pub solver_seed: u32,
    pub solver: String,
    pub case_name: String,
    pub model_index: usize,
    /// Full realised choice indices and labels, rather than the replay prefix.
    pub choice_path: Vec<usize>,
    pub choices: Vec<String>,
    /// Distinct representation rules present in the compiled model.
    pub representations: Vec<String>,
    pub minion_discrete_threshold: Option<usize>,
    pub sat_encoding_amo: Vec<String>,
    pub sat_encoding_cardinality: Vec<String>,
    pub sat_encoding_pb: Vec<String>,
    pub sat_encoding_table: Vec<String>,
    pub sat_encoding_element: Vec<String>,
    pub sat_encoding_alldifferent: Vec<String>,
    pub status: String,
    /// Zero means all solutions; absent means translation only.
    pub solution_limit: Option<i32>,
    /// Wall time for parsing, instantiation and rewriting.
    pub translation_time: Option<f64>,
    /// Wall time for complete solution collection and output.
    pub solve_time: Option<f64>,
    pub backend_time: Option<f64>,
    /// Solver-call wall time, including callbacks and solver-time rewrites.
    pub search_time: Option<f64>,
    pub solution_processing_time: Option<f64>,
    /// Rule counts for this compiled model; written as dotted keys by the canonical writer.
    #[serde(skip_serializing)]
    pub rule_trace: RecordedRuleTraceStats,
}

/// Recorded rule counts for one compiled model.
#[derive(Deserialize, Debug, Default)]
#[serde(default)]
#[serde(deny_unknown_fields)]
pub struct RecordedRuleTraceStats {
    #[serde(rename = "attempts")]
    pub total_rule_attempts: Option<u64>,
    #[serde(rename = "applications")]
    pub total_rule_applications: Option<u64>,
    pub rules: std::collections::BTreeMap<String, u64>,
}

pub fn read_stats_or_default(path: &Path) -> io::Result<TestRunStats> {
    if path.exists() {
        let contents = fs::read_to_string(path)?;
        toml::from_str(&contents).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    } else {
        Ok(TestRunStats::default())
    }
}

/// Recorded status and timings for one implementation in a test config.
#[derive(Deserialize, Debug, Default)]
#[serde(default)]
#[serde(deny_unknown_fields)]
pub struct RecordedToolStats {
    /// Latest observed status, such as `ok`, `fail`, or `timeout(N)`.
    pub status: Option<String>,

    /// Translation time in seconds.
    #[serde(rename = "translation-time")]
    pub translation_time: Option<f64>,

    /// Solver time in seconds.
    #[serde(rename = "solve-time")]
    pub solve_time: Option<f64>,

    /// Wall-clock time of the complete tool command, in seconds, when available.
    #[serde(rename = "wall-clock-time")]
    pub wall_clock_time: Option<f64>,

    /// Conjure-only translation time in seconds, when available.
    #[serde(rename = "conjure-translation-time")]
    pub conjure_translation_time: Option<f64>,

    /// Savile Row translation time in seconds, when available.
    #[serde(rename = "savilerow-translation-time")]
    pub savilerow_translation_time: Option<f64>,
}

/// Replace this configuration's model rows, removing stale portfolio entries.
pub fn replace_config_run_stats(
    path: &Path,
    config: &RecordedRunConfig,
    rows: Vec<RecordedRunStats>,
) -> io::Result<()> {
    update_canonical_stats(path, |stats| {
        stats.runs.retain(|row| {
            !(row.parser == config.parser
                && row.rewriter == config.rewriter
                && row.comprehension_expander == config.comprehension_expander
                && row.heuristic == config.heuristic
                && row.channelling == config.channelling
                && row.seed == config.seed
                && row.solver_seed == config.solver_seed
                && row.solver == config.solver)
        });
        stats.runs.extend(rows);
        stats.runs.sort_by(|a, b| {
            (
                &a.parser,
                &a.rewriter,
                &a.comprehension_expander,
                &a.heuristic,
                &a.channelling,
                a.seed,
                a.solver_seed,
                &a.solver,
                a.model_index,
            )
                .cmp(&(
                    &b.parser,
                    &b.rewriter,
                    &b.comprehension_expander,
                    &b.heuristic,
                    &b.channelling,
                    b.seed,
                    b.solver_seed,
                    &b.solver,
                    b.model_index,
                ))
        });
    })
}

pub fn update_canonical_stats(
    path: &Path,
    update: impl FnOnce(&mut TestRunStats),
) -> io::Result<()> {
    let mut stats = read_stats_or_default(path)?;
    update(&mut stats);
    write_canonical_stats(path, &stats)
}

fn push_optional_float(contents: &mut String, key: &str, value: Option<f64>) {
    if let Some(value) = value {
        contents.push_str(key);
        contents.push_str(" = ");
        contents.push_str(&canonical_float(value));
        contents.push('\n');
    }
}

pub fn write_canonical_stats(path: &Path, stats: &TestRunStats) -> io::Result<()> {
    let mut contents = String::new();

    if let Some(expected_time) = stats.expected_time {
        contents.push_str(&format!("expected-time = {expected_time}\n"));
    }
    if let Some(status) = &stats.status {
        contents.push_str(&format!("status = {}\n", quoted_toml_string(status)));
    }

    let conjure = &stats.conjure;
    let has_conjure_stats = conjure.status.is_some()
        || conjure.translation_time.is_some()
        || conjure.solve_time.is_some()
        || conjure.wall_clock_time.is_some()
        || conjure.conjure_translation_time.is_some()
        || conjure.savilerow_translation_time.is_some();
    if has_conjure_stats {
        if !contents.is_empty() {
            contents.push('\n');
        }
        contents.push_str("[conjure]\n");
        if let Some(status) = &conjure.status {
            contents.push_str(&format!("status = {}\n", quoted_toml_string(status)));
        }
        push_optional_float(&mut contents, "wall-clock-time", conjure.wall_clock_time);
        push_optional_float(&mut contents, "translation-time", conjure.translation_time);
        push_optional_float(
            &mut contents,
            "conjure-translation-time",
            conjure.conjure_translation_time,
        );
        push_optional_float(
            &mut contents,
            "savilerow-translation-time",
            conjure.savilerow_translation_time,
        );
        push_optional_float(&mut contents, "solve-time", conjure.solve_time);
    }

    for run in &stats.runs {
        contents.push_str("\n[[runs]]\n");
        contents.push_str(
            &toml::to_string(run).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?,
        );

        if run.rule_trace.total_rule_attempts.is_some()
            || run.rule_trace.total_rule_applications.is_some()
            || !run.rule_trace.rules.is_empty()
        {
            contents.push('\n');
        }
        if let Some(attempts) = run.rule_trace.total_rule_attempts {
            contents.push_str(&format!("rule-trace.attempts = {attempts}\n"));
        }
        if let Some(applications) = run.rule_trace.total_rule_applications {
            contents.push_str(&format!("rule-trace.applications = {applications}\n"));
        }
        for (rule, count) in rule_trace_rules_by_count_desc(&run.rule_trace.rules) {
            contents.push_str(&format!(
                "rule-trace.rules.{} = {count}\n",
                canonical_rule_key(rule)
            ));
        }
    }

    fs::write(path, contents)
}

#![allow(unused)]

use conjure_cp::settings::{
    Channelling, Heuristic, Parser, QuantifiedExpander, Rewriter, SolverFamily,
};
use serde::Deserialize;
use serde::de::{self, Visitor};
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use toml_edit::{DocumentMut, Item, Table, value};

pub const STATS_FILE_NAME: &str = "stats.toml";

pub fn stats_path(test_dir: &Path) -> std::path::PathBuf {
    test_dir.join(STATS_FILE_NAME)
}

/// Starts a fresh stats snapshot for an integration run.
pub fn reset_stats_for_run(path: &Path) -> io::Result<()> {
    write_canonical_stats(path, &TestRunStats::default())
}

fn read_toml_document_or_empty(path: &Path) -> io::Result<DocumentMut> {
    if path.exists() {
        let contents = fs::read_to_string(path)?;
        if contents.trim().is_empty() {
            Ok(DocumentMut::new())
        } else {
            contents
                .parse::<DocumentMut>()
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
        }
    } else {
        Ok(DocumentMut::new())
    }
}

fn write_toml_document(path: &Path, document: &DocumentMut) -> io::Result<()> {
    let mut contents = document.to_string();
    contents.truncate(contents.trim_end_matches('\n').len());
    contents.push('\n');
    fs::write(path, contents)
}

fn quoted_toml_string(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

fn canonical_float(value: f64) -> String {
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

// toml_edit's Index impl panics on missing keys, so use .get() before creating tables.
fn ensure_table(document: &mut DocumentMut, key: &str) {
    if document.get(key).is_some_and(|item| item.is_table()) {
        return;
    }
    document[key] = Item::Table(Table::new());
}

fn ensure_nested_table(document: &mut DocumentMut, keys: &[&str]) {
    let (head, tail) = keys.split_first().expect("table path must not be empty");
    ensure_table(document, head);
    let mut table = document[head].as_table_mut().expect("table exists");
    for key in tail {
        if table.get(key).is_some_and(|item| item.is_table()) {
            table = table[key].as_table_mut().expect("table exists");
            continue;
        }
        table[*key] = Item::Table(Table::new());
        table = table[key].as_table_mut().expect("table exists");
    }
}

fn parse_values<T>(values: &[String]) -> Result<Vec<T>, String>
where
    T: FromStr<Err = String>,
{
    values.iter().map(|value| value.parse()).collect()
}

fn deserialize_string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        String(String),
        Vec(Vec<String>),
    }

    Ok(match StringOrVec::deserialize(deserializer)? {
        StringOrVec::String(s) => vec![s],
        StringOrVec::Vec(v) => v,
    })
}

fn default_skip_conjure_validation() -> String {
    String::new()
}

fn default_skip() -> String {
    String::new()
}

fn default_minion_discrete_threshold() -> usize {
    conjure_cp::settings::DEFAULT_MINION_DISCRETE_THRESHOLD
}

fn deserialise_expected_time<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<u64>::deserialize(deserializer)
}

fn default_number_of_solutions() -> NumberOfSolutions {
    NumberOfSolutions::All
}

fn default_keep_intermediate_solutions() -> bool {
    false
}

fn deserialise_number_of_solutions<'de, D>(deserializer: D) -> Result<NumberOfSolutions, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserializer.deserialize_any(NumberOfSolutionsVisitor)
}

struct NumberOfSolutionsVisitor;

impl<'de> Visitor<'de> for NumberOfSolutionsVisitor {
    type Value = NumberOfSolutions;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a positive integer, the string \"all\", or the string \"skip\"")
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let value = i32::try_from(value).map_err(|err| {
            E::custom(format!(
                "number-of-solutions is too large for the solver limit: {err}"
            ))
        })?;

        if value == 0 {
            return Err(E::custom(
                "number-of-solutions must be positive, or the string \"all\" or \"skip\"",
            ));
        }

        Ok(NumberOfSolutions::Limit(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let value = u64::try_from(value).map_err(|_| {
            E::custom("number-of-solutions must be positive, or the string \"all\" or \"skip\"")
        })?;

        self.visit_u64(value)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match value {
            "all" => Ok(NumberOfSolutions::All),
            "skip" => Ok(NumberOfSolutions::Skip),
            _ => Err(E::custom(format!(
                "invalid number-of-solutions value '{value}', expected a positive integer, \"all\", or \"skip\""
            ))),
        }
    }
}

/// Rounds an observed runtime into the coarse `expected-time` buckets used by test configs,
/// such as `1`, `5`, `10`, `30`, `60`, and so on.
pub fn round_expected_time(duration: Duration) -> u64 {
    let seconds = duration.as_secs_f64();

    if seconds <= 1.0 {
        1
    } else if seconds <= 5.0 {
        5
    } else if seconds <= 10.0 {
        10
    } else {
        ((seconds / 30.0).ceil() as u64) * 30
    }
}

/// Inserts or updates the `expected-time` entry in a test `stats.toml`.
pub fn upsert_expected_time_stats(path: &Path, expected_time: u64) -> io::Result<()> {
    update_canonical_stats(path, |stats| stats.expected_time = Some(expected_time))
}

/// Inserts or updates the `expected-time` entry in a test `config.toml`.
///
/// Custom tests still keep their expected-time metadata in `config.toml`; integration tests use
/// `stats.toml`.
pub fn upsert_expected_time_config(path: &Path, expected_time: u64) -> io::Result<()> {
    let expected_time = i64::try_from(expected_time).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("expected-time is too large to write to TOML: {err}"),
        )
    })?;
    let mut document = read_toml_document_or_empty(path)?;
    document["expected-time"] = value(expected_time);
    write_toml_document(path, &document)
}

/// Inserts or updates the latest observed integration status in a test `stats.toml`.
pub fn upsert_status_stats(path: &Path, status: &str) -> io::Result<()> {
    update_canonical_stats(path, |stats| stats.status = Some(status.to_string()))
}

/// Inserts or updates the latest observed status for one part of an integration test.
pub fn upsert_tool_status_stats(path: &Path, tool: &str, status: &str) -> io::Result<()> {
    if tool != "conjure" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("'{tool}' does not have shared tool stats"),
        ));
    }
    update_canonical_stats(path, |stats| {
        stats.conjure.status = Some(status.to_string());
    })
}

/// Identifies one configured integration-test run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedRunConfig {
    pub parser: String,
    pub rewriter: String,
    pub comprehension_expander: String,
    pub heuristic: String,
    pub channelling: String,
    pub seed: u64,
    pub solver: String,
}

/// Inserts or updates the latest observed status for one integration-test configuration.
pub fn upsert_config_status_stats(
    path: &Path,
    config: &RecordedRunConfig,
    status: &str,
) -> io::Result<()> {
    update_canonical_stats(path, |stats| {
        config_run_mut(stats, config).status = Some(status.to_string());
    })
}

/// Inserts or updates the conjure-oxide timing stats for one integration-test configuration.
pub fn upsert_config_oxide_timing_stats(
    path: &Path,
    config: &RecordedRunConfig,
    translation_time: f64,
    solve_time: Option<f64>,
) -> io::Result<()> {
    update_canonical_stats(path, |stats| {
        let run = config_run_mut(stats, config);
        run.translation_time = Some(translation_time);
        if let Some(solve_time) = solve_time {
            run.solve_time = Some(solve_time);
        }
    })
}

/// Timing measurements recorded from one accepted Conjure reference run.
#[derive(Clone, Copy, Debug)]
pub struct RecordedConjureStats {
    /// Wall-clock time of the complete `conjure solve` command, in seconds.
    pub conjure_wall_clock_time: f64,
    /// Total Conjure plus Savile Row translation time, in seconds.
    pub conjure_translation_time: f64,
    /// Time spent by Conjure before Savile Row is invoked, in seconds.
    pub conjure_driver_translation_time: f64,
    /// Time spent by Savile Row during reference translation, in seconds.
    pub savilerow_translation_time: f64,
    /// Time spent solving the Conjure plus Savile Row reference model, in seconds.
    pub conjure_solve_time: f64,
}

/// Inserts or updates the recorded Conjure reference timings in a test `stats.toml`.
pub fn upsert_conjure_timing_stats(path: &Path, stats: RecordedConjureStats) -> io::Result<()> {
    update_canonical_stats(path, |current| {
        current.conjure.wall_clock_time = Some(stats.conjure_wall_clock_time);
        current.conjure.translation_time = Some(stats.conjure_translation_time);
        current.conjure.conjure_translation_time = Some(stats.conjure_driver_translation_time);
        current.conjure.savilerow_translation_time = Some(stats.savilerow_translation_time);
        current.conjure.solve_time = Some(stats.conjure_solve_time);
    })
}

/// Aggregated rule application counts for the expected rule traces in one integration test.
#[derive(Clone, Debug, Default)]
pub struct RuleTraceAggregateStats {
    pub total_rule_attempts: u64,
    pub total_rule_applications: u64,
    pub rules: std::collections::BTreeMap<String, u64>,
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

/// Replaces the rule-trace aggregates for one integration-test configuration.
pub fn upsert_config_rule_trace_aggregate_stats(
    path: &Path,
    config: &RecordedRunConfig,
    aggregates: &RuleTraceAggregateStats,
) -> io::Result<()> {
    update_canonical_stats(path, |stats| {
        let rule_trace = &mut config_run_mut(stats, config).rule_trace;
        rule_trace.total_rule_attempts = Some(aggregates.total_rule_attempts);
        rule_trace.total_rule_applications = Some(aggregates.total_rule_applications);
        rule_trace.rules.clone_from(&aggregates.rules);
    })
}

/// Recorded integration-run metadata for one test directory.
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

    /// Canonical per-configuration run records.
    pub runs: Vec<RecordedConfigRunStats>,
}

/// Canonical metadata for one integration-test configuration.
#[derive(Deserialize, Debug, Default)]
#[serde(default)]
#[serde(deny_unknown_fields)]
pub struct RecordedConfigRunStats {
    pub parser: String,
    pub rewriter: String,
    #[serde(rename = "comprehension-expander")]
    pub comprehension_expander: String,
    pub heuristic: String,
    pub channelling: String,
    pub seed: Option<u64>,
    pub solver: String,
    pub status: Option<String>,
    #[serde(rename = "translation-time")]
    pub translation_time: Option<f64>,
    #[serde(rename = "solve-time")]
    pub solve_time: Option<f64>,
    #[serde(rename = "rule-trace")]
    pub rule_trace: RecordedRuleTraceStats,
}

/// Recorded rule-trace aggregate metadata for one test directory.
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

fn config_run_mut<'a>(
    stats: &'a mut TestRunStats,
    config: &RecordedRunConfig,
) -> &'a mut RecordedConfigRunStats {
    if let Some(index) = stats.runs.iter().position(|run| {
        run.parser == config.parser
            && run.rewriter == config.rewriter
            && run.comprehension_expander == config.comprehension_expander
            && run.heuristic == config.heuristic
            && run.channelling == config.channelling
            && run.seed == Some(config.seed)
            && run.solver == config.solver
    }) {
        return &mut stats.runs[index];
    }

    stats.runs.push(RecordedConfigRunStats {
        parser: config.parser.clone(),
        rewriter: config.rewriter.clone(),
        comprehension_expander: config.comprehension_expander.clone(),
        heuristic: config.heuristic.clone(),
        channelling: config.channelling.clone(),
        seed: Some(config.seed),
        solver: config.solver.clone(),
        ..RecordedConfigRunStats::default()
    });
    stats.runs.last_mut().expect("run was just inserted")
}

fn update_canonical_stats(path: &Path, update: impl FnOnce(&mut TestRunStats)) -> io::Result<()> {
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

fn write_canonical_stats(path: &Path, stats: &TestRunStats) -> io::Result<()> {
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
        if !contents.is_empty() {
            contents.push('\n');
        }
        contents.push_str("[[runs]]\n");
        contents.push_str(&format!(
            "comprehension-expander = {}\n",
            quoted_toml_string(&run.comprehension_expander)
        ));
        if !run.parser.is_empty() {
            contents.push_str(&format!("parser = {}\n", quoted_toml_string(&run.parser)));
        }
        if !run.rewriter.is_empty() {
            contents.push_str(&format!(
                "rewriter = {}\n",
                quoted_toml_string(&run.rewriter)
            ));
        }
        if !run.heuristic.is_empty() {
            contents.push_str(&format!(
                "heuristic = {}\n",
                quoted_toml_string(&run.heuristic)
            ));
        }
        if !run.channelling.is_empty() {
            contents.push_str(&format!(
                "channelling = {}\n",
                quoted_toml_string(&run.channelling)
            ));
        }
        if let Some(seed) = run.seed {
            contents.push_str(&format!("seed = {seed}\n"));
        }
        if !run.solver.is_empty() {
            contents.push_str(&format!("solver = {}\n", quoted_toml_string(&run.solver)));
        }
        if let Some(status) = &run.status {
            contents.push_str(&format!("status = {}\n", quoted_toml_string(status)));
        }
        push_optional_float(&mut contents, "translation-time", run.translation_time);
        push_optional_float(&mut contents, "solve-time", run.solve_time);

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

/// Solution search limit requested by an integration test.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumberOfSolutions {
    /// Search for every solution.
    All,
    /// Stop after the given number of solutions.
    Limit(i32),
    /// Do not run the solver for this integration test.
    Skip,
}

impl NumberOfSolutions {
    /// Converts the config value into the solver API limit, where `0` means all solutions.
    pub fn as_solver_limit(self) -> Option<i32> {
        match self {
            Self::All => Some(0),
            Self::Limit(limit) => Some(limit),
            Self::Skip => None,
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(default)]
#[serde(deny_unknown_fields)]
pub struct TestConfig {
    #[serde(
        default,
        rename = "parser",
        deserialize_with = "deserialize_string_or_vec"
    )]
    pub parser: Vec<String>, // Stage 1a: list of parsers (tree-sitter or via-conjure)

    #[serde(
        default,
        rename = "rewriter",
        deserialize_with = "deserialize_string_or_vec"
    )]
    pub rewriter: Vec<String>,
    #[serde(
        default,
        rename = "comprehension-expander",
        deserialize_with = "deserialize_string_or_vec"
    )]
    pub comprehension_expander: Vec<String>,
    #[serde(
        default,
        rename = "heuristic",
        deserialize_with = "deserialize_string_or_vec"
    )]
    pub heuristic: Vec<String>,
    #[serde(
        default,
        rename = "channelling",
        deserialize_with = "deserialize_string_or_vec"
    )]
    pub channelling: Vec<String>,
    #[serde(default, rename = "seed")]
    pub seed: u64,
    #[serde(
        default,
        rename = "solver",
        deserialize_with = "deserialize_string_or_vec"
    )]
    pub solver: Vec<String>,

    #[serde(
        default,
        rename = "extra-rule-sets",
        deserialize_with = "deserialize_string_or_vec"
    )]
    pub extra_rule_sets: Vec<String>,

    #[serde(
        default = "default_minion_discrete_threshold",
        rename = "minion-discrete-threshold"
    )]
    pub minion_discrete_threshold: usize,

    #[serde(
        default = "default_skip_conjure_validation",
        rename = "skip-conjure-validation"
    )]
    pub skip_conjure_validation: String,

    #[serde(
        default = "default_number_of_solutions",
        rename = "number-of-solutions",
        deserialize_with = "deserialise_number_of_solutions"
    )]
    pub number_of_solutions: NumberOfSolutions,

    #[serde(
        default = "default_keep_intermediate_solutions",
        rename = "keep-intermediate-solutions"
    )]
    pub keep_intermediate_solutions: bool,

    /// Empty `skip` runs the test; a non-empty string ignores it and records why.
    #[serde(default = "default_skip")]
    pub skip: String,

    #[serde(
        default,
        rename = "expected-time",
        deserialize_with = "deserialise_expected_time"
    )]
    pub expected_time: Option<u64>,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            skip: String::new(),
            parser: vec!["tree-sitter".to_string(), "via-conjure".to_string()],
            rewriter: vec!["optimised".to_string()],
            comprehension_expander: vec!["auto".to_string()],
            heuristic: vec!["x".to_string()],
            channelling: vec!["no".to_string()],
            seed: 0,
            solver: {
                let mut solvers = vec![
                    "minion".to_string(),
                    "sat-log".to_string(),
                    "sat-direct".to_string(),
                    "sat-order".to_string(),
                ];

                {
                    solvers.extend([
                        "smt".to_string(),
                        "smt-lia-arrays-nodiscrete".to_string(),
                        "smt-lia-atomic".to_string(),
                        "smt-lia-atomic-nodiscrete".to_string(),
                        "smt-bv-arrays".to_string(),
                        "smt-bv-arrays-nodiscrete".to_string(),
                        "smt-bv-atomic".to_string(),
                        "smt-bv-atomic-nodiscrete".to_string(),
                    ]);
                }
                solvers
            },
            extra_rule_sets: Vec::new(),
            minion_discrete_threshold: default_minion_discrete_threshold(),
            skip_conjure_validation: String::new(),
            number_of_solutions: NumberOfSolutions::All,
            keep_intermediate_solutions: false,
            expected_time: None,
        }
    }
}

impl TestConfig {
    /// Empty `skip-conjure-validation` runs Conjure reference validation during accept.
    pub fn should_skip_conjure_validation(&self) -> bool {
        !self.skip_conjure_validation.is_empty()
    }

    /// Empty `skip` runs the test; a non-empty string ignores it.
    pub fn should_skip(&self) -> bool {
        !self.skip.is_empty()
    }

    pub fn skip_reason(&self) -> Option<&str> {
        if self.skip.is_empty() {
            None
        } else {
            Some(self.skip.as_str())
        }
    }

    pub fn configured_parsers(&self) -> Result<Vec<Parser>, String> {
        parse_values(&self.parser)
    }

    pub fn configured_rewriters(&self) -> Result<Vec<Rewriter>, String> {
        if self.rewriter.is_empty() {
            return Err("setting 'rewriter' has no values".to_string());
        }

        parse_values(&self.rewriter)
    }

    pub fn configured_comprehension_expanders(&self) -> Result<Vec<QuantifiedExpander>, String> {
        let values = if self.comprehension_expander.is_empty() {
            vec!["auto".to_string()]
        } else {
            self.comprehension_expander.clone()
        };

        parse_values(&values)
    }

    pub fn configured_heuristics(&self) -> Result<Vec<Heuristic>, String> {
        let values = if self.heuristic.is_empty() {
            vec!["x".to_string()]
        } else {
            self.heuristic.clone()
        };
        parse_values(&values)
    }

    pub fn configured_channelling(&self) -> Result<Vec<Channelling>, String> {
        let values = if self.channelling.is_empty() {
            vec!["no".to_string()]
        } else {
            self.channelling.clone()
        };
        let configured: Vec<Channelling> = parse_values(&values)?;
        if configured.contains(&Channelling::Yes) {
            return Err("setting 'channelling=yes' is not supported yet".to_string());
        }
        Ok(configured)
    }

    pub fn configured_solvers(&self) -> Result<Vec<SolverFamily>, String> {
        parse_values(&self.solver)
    }

    pub fn uses_smt_solver(&self) -> bool {
        self.solver
            .iter()
            .any(|solver| solver == "smt" || solver.starts_with("smt-"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_runs_in_the_canonical_shape() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(STATS_FILE_NAME);
        reset_stats_for_run(&path).unwrap();
        upsert_expected_time_stats(&path, 1).unwrap();
        upsert_status_stats(&path, "ok").unwrap();
        upsert_tool_status_stats(&path, "conjure", "ok").unwrap();

        upsert_conjure_timing_stats(
            &path,
            RecordedConjureStats {
                conjure_wall_clock_time: 0.572383542,
                conjure_translation_time: 0.572305542,
                conjure_driver_translation_time: 0.566305542,
                savilerow_translation_time: 0.006,
                conjure_solve_time: 0.000078,
            },
        )
        .unwrap();

        for (solver, translation_time) in [("minion", 1.0), ("sat-log", 2.0)] {
            let config = RecordedRunConfig {
                parser: "tree-sitter".to_string(),
                rewriter: "optimised".to_string(),
                comprehension_expander: "auto".to_string(),
                heuristic: "x".to_string(),
                channelling: "no".to_string(),
                seed: 0,
                solver: solver.to_string(),
            };
            upsert_config_status_stats(&path, &config, "ok").unwrap();
            upsert_config_oxide_timing_stats(&path, &config, translation_time, Some(3.0)).unwrap();
            upsert_config_rule_trace_aggregate_stats(
                &path,
                &config,
                &RuleTraceAggregateStats {
                    total_rule_attempts: 4,
                    total_rule_applications: 5,
                    rules: std::collections::BTreeMap::from([("rule_name".to_string(), 5)]),
                },
            )
            .unwrap();
        }

        let expected = r#"expected-time = 1
status = "ok"

[conjure]
status = "ok"
wall-clock-time = 0.572384
translation-time = 0.572306
conjure-translation-time = 0.566306
savilerow-translation-time = 0.006
solve-time = 0.000078

[[runs]]
comprehension-expander = "auto"
parser = "tree-sitter"
rewriter = "optimised"
heuristic = "x"
channelling = "no"
seed = 0
solver = "minion"
status = "ok"
translation-time = 1.0
solve-time = 3.0

rule-trace.attempts = 4
rule-trace.applications = 5
rule-trace.rules.rule_name = 5

[[runs]]
comprehension-expander = "auto"
parser = "tree-sitter"
rewriter = "optimised"
heuristic = "x"
channelling = "no"
seed = 0
solver = "sat-log"
status = "ok"
translation-time = 2.0
solve-time = 3.0

rule-trace.attempts = 4
rule-trace.applications = 5
rule-trace.rules.rule_name = 5
"#;
        assert_eq!(fs::read_to_string(&path).unwrap(), expected);

        let stats = read_stats_or_default(&path).unwrap();
        assert_eq!(stats.runs.len(), 2);
    }

    #[test]
    fn resetting_stats_discards_the_previous_snapshot() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(STATS_FILE_NAME);
        fs::write(
            &path,
            "status = \"fail\"\n\n[[runs]]\ncomprehension-expander = \"native\"\nstatus = \"fail\"\n",
        )
        .unwrap();

        let previous = read_stats_or_default(&path).unwrap();
        assert_eq!(previous.runs[0].solver, "");

        reset_stats_for_run(&path).unwrap();

        assert_eq!(fs::read_to_string(path).unwrap(), "");
    }
}

#![allow(unused)]

use conjure_cp::settings::{Parser, QuantifiedExpander, Rewriter, SolverFamily};
use serde::Deserialize;
use serde::de::{self, Visitor};
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use toml_edit::{DocumentMut, Item, Table, value};

use crate::text_files::write_text_with_trailing_newline;

pub const STATS_FILE_NAME: &str = "stats.toml";

pub fn stats_path(test_dir: &Path) -> std::path::PathBuf {
    test_dir.join(STATS_FILE_NAME)
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
    write_text_with_trailing_newline(path, &document.to_string())
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
        formatter.write_str("a positive integer or the string \"all\"")
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
                "number-of-solutions must be positive, or the string \"all\"",
            ));
        }

        Ok(NumberOfSolutions::Limit(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let value = u64::try_from(value).map_err(|_| {
            E::custom("number-of-solutions must be positive, or the string \"all\"")
        })?;

        self.visit_u64(value)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value == "all" {
            Ok(NumberOfSolutions::All)
        } else {
            Err(E::custom(format!(
                "invalid number-of-solutions value '{value}', expected a positive integer or \"all\""
            )))
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

/// Inserts or updates the `expected-time` entry in a test `config.toml`.
///
/// Custom tests still keep their expected-time metadata in `config.toml`; integration tests use
/// `stats.toml`.
pub fn upsert_expected_time_config(path: &Path, expected_time: u64) -> io::Result<()> {
    upsert_expected_time_stats(path, expected_time)
}

/// Inserts or updates the latest observed integration status in a test `stats.toml`.
pub fn upsert_status_stats(path: &Path, status: &str) -> io::Result<()> {
    let mut document = read_toml_document_or_empty(path)?;
    document["status"] = value(status);

    write_toml_document(path, &document)
}

/// Inserts or updates the latest observed status for one part of an integration test.
pub fn upsert_tool_status_stats(path: &Path, tool: &str, status: &str) -> io::Result<()> {
    let mut document = read_toml_document_or_empty(path)?;
    ensure_table(&mut document, tool);
    document[tool]["status"] = value(status);

    write_toml_document(path, &document)
}

/// Timing measurements recorded from one accepted integration test run.
#[derive(Clone, Copy, Debug)]
pub struct RecordedRunStats {
    /// Time spent translating the model through conjure-oxide, in seconds.
    pub oxide_translation_time: f64,
    /// Time spent solving through conjure-oxide's configured solver, in seconds.
    pub oxide_solve_time: f64,
    /// Total Conjure plus Savile Row translation time, in seconds.
    pub conjure_translation_time: f64,
    /// Time spent by Conjure before Savile Row is invoked, in seconds.
    pub conjure_driver_translation_time: f64,
    /// Time spent by Savile Row during reference translation, in seconds.
    pub savilerow_translation_time: f64,
    /// Time spent solving the Conjure plus Savile Row reference model, in seconds.
    pub conjure_solve_time: f64,
}

/// Inserts or updates the recorded timing stats in a test `stats.toml`.
pub fn upsert_recorded_run_stats(path: &Path, stats: RecordedRunStats) -> io::Result<()> {
    let mut document = read_toml_document_or_empty(path)?;

    ensure_table(&mut document, "oxide");
    document["oxide"]["translation-time"] = value(stats.oxide_translation_time);
    document["oxide"]["solve-time"] = value(stats.oxide_solve_time);

    ensure_table(&mut document, "conjure");
    document["conjure"]["translation-time"] = value(stats.conjure_translation_time);
    document["conjure"]["conjure-translation-time"] = value(stats.conjure_driver_translation_time);
    document["conjure"]["savilerow-translation-time"] = value(stats.savilerow_translation_time);
    document["conjure"]["solve-time"] = value(stats.conjure_solve_time);

    write_toml_document(path, &document)
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

/// Replaces the recorded rule trace aggregates in a test `stats.toml`.
pub fn upsert_rule_trace_aggregate_stats(
    path: &Path,
    aggregates: &RuleTraceAggregateStats,
) -> io::Result<()> {
    let mut document = read_toml_document_or_empty(path)?;

    ensure_nested_table(&mut document, &["rule-trace", "rules"]);
    document["rule-trace"]["total-rule-attempts"] = value(
        i64::try_from(aggregates.total_rule_attempts).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("rule trace attempt count is too large to write to TOML: {err}"),
            )
        })?,
    );
    document["rule-trace"]["total-rule-applications"] = value(
        i64::try_from(aggregates.total_rule_applications).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("rule trace application count is too large to write to TOML: {err}"),
            )
        })?,
    );

    let rules = document["rule-trace"]["rules"]
        .as_table_mut()
        .expect("rule trace rules table exists");
    rules.clear();

    for (rule, count) in rule_trace_rules_by_count_desc(&aggregates.rules) {
        rules[rule.as_str()] = value(i64::try_from(count).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("rule trace count for '{rule}' is too large to write to TOML: {err}"),
            )
        })?);
    }

    write_toml_document(path, &document)
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

    /// Metadata recorded for conjure-oxide.
    pub oxide: RecordedToolStats,
    /// Metadata recorded for the Conjure plus Savile Row reference run.
    pub conjure: RecordedToolStats,

    /// Aggregated data derived from expected rule traces.
    #[serde(rename = "rule-trace")]
    pub rule_trace: RecordedRuleTraceStats,
}

/// Recorded rule-trace aggregate metadata for one test directory.
#[derive(Deserialize, Debug, Default)]
#[serde(default)]
#[serde(deny_unknown_fields)]
pub struct RecordedRuleTraceStats {
    #[serde(rename = "total-rule-attempts")]
    pub total_rule_attempts: Option<u64>,
    #[serde(rename = "total-rule-applications")]
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

    /// Conjure-only translation time in seconds, when available.
    #[serde(rename = "conjure-translation-time")]
    pub conjure_translation_time: Option<f64>,

    /// Savile Row translation time in seconds, when available.
    #[serde(rename = "savilerow-translation-time")]
    pub savilerow_translation_time: Option<f64>,
}

/// Solution search limit requested by an integration test.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumberOfSolutions {
    /// Search for every solution.
    All,
    /// Stop after the given number of solutions.
    Limit(i32),
}

impl NumberOfSolutions {
    /// Converts the config value into the solver API limit, where `0` means all solutions.
    pub fn as_solver_limit(self) -> i32 {
        match self {
            Self::All => 0,
            Self::Limit(limit) => limit,
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
        rename = "solver",
        deserialize_with = "deserialize_string_or_vec"
    )]
    pub solver: Vec<String>,

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
            comprehension_expander: vec![
                "native".to_string(),
                "via-solver".to_string(),
                "via-solver-ac".to_string(),
            ],
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
            vec!["native".to_string()]
        } else {
            self.comprehension_expander.clone()
        };

        parse_values(&values)
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

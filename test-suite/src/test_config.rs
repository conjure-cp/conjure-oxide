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
        if table.get(*key).is_some_and(|item| item.is_table()) {
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

/// Inserts or updates the `expected-time` entry in a test `config.toml`.
pub fn upsert_expected_time_config(path: &Path, expected_time: u64) -> io::Result<()> {
    let expected_time = i64::try_from(expected_time).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("expected-time is too large to write to TOML: {err}"),
        )
    })?;

    let mut document = if path.exists() {
        let contents = fs::read_to_string(path)?;
        if contents.trim().is_empty() {
            DocumentMut::new()
        } else {
            contents
                .parse::<DocumentMut>()
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?
        }
    } else {
        DocumentMut::new()
    };

    document["expected-time"] = value(expected_time);

    write_toml_document(path, &document)
}

/// Inserts or updates the latest observed integration status in a test `config.toml`.
pub fn upsert_status_config(path: &Path, status: &str) -> io::Result<()> {
    let mut document = if path.exists() {
        let contents = fs::read_to_string(path)?;
        if contents.trim().is_empty() {
            DocumentMut::new()
        } else {
            contents
                .parse::<DocumentMut>()
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?
        }
    } else {
        DocumentMut::new()
    };

    document["status"] = value(status);

    write_toml_document(path, &document)
}

/// Inserts or updates the latest observed status for one part of an integration test.
pub fn upsert_tool_status_config(path: &Path, tool: &str, status: &str) -> io::Result<()> {
    let mut document = if path.exists() {
        let contents = fs::read_to_string(path)?;
        if contents.trim().is_empty() {
            DocumentMut::new()
        } else {
            contents
                .parse::<DocumentMut>()
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?
        }
    } else {
        DocumentMut::new()
    };

    ensure_nested_table(&mut document, &["stats", tool]);
    document["stats"][tool]["status"] = value(status);

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

/// Inserts or updates the recorded timing stats in a test `config.toml`.
pub fn upsert_recorded_run_stats_config(path: &Path, stats: RecordedRunStats) -> io::Result<()> {
    let mut document = if path.exists() {
        let contents = fs::read_to_string(path)?;
        if contents.trim().is_empty() {
            DocumentMut::new()
        } else {
            contents
                .parse::<DocumentMut>()
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?
        }
    } else {
        DocumentMut::new()
    };

    ensure_nested_table(&mut document, &["stats", "oxide"]);
    document["stats"]["oxide"]["translation-time"] = value(stats.oxide_translation_time);
    document["stats"]["oxide"]["solve-time"] = value(stats.oxide_solve_time);

    ensure_nested_table(&mut document, &["stats", "conjure"]);
    document["stats"]["conjure"]["translation-time"] = value(stats.conjure_translation_time);
    document["stats"]["conjure"]["conjure-translation-time"] =
        value(stats.conjure_driver_translation_time);
    document["stats"]["conjure"]["savilerow-translation-time"] =
        value(stats.savilerow_translation_time);
    document["stats"]["conjure"]["solve-time"] = value(stats.conjure_solve_time);

    write_toml_document(path, &document)
}

/// Recorded integration-run metadata grouped by implementation.
#[derive(Deserialize, Debug, Default)]
#[serde(default)]
#[serde(deny_unknown_fields)]
pub struct TestStats {
    /// Metadata recorded for conjure-oxide.
    pub oxide: RecordedToolStats,
    /// Metadata recorded for the Conjure plus Savile Row reference run.
    pub conjure: RecordedToolStats,
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

    #[serde(default = "default_skip_conjure_validation", rename = "skip-conjure-validation")]
    pub skip_conjure_validation: String,

    #[serde(
        default = "default_number_of_solutions",
        rename = "number-of-solutions",
        deserialize_with = "deserialise_number_of_solutions"
    )]
    pub number_of_solutions: NumberOfSolutions,

    pub status: Option<String>,

    // Generate this test but do not run it
    pub skip: bool,

    #[serde(
        default,
        rename = "expected-time",
        deserialize_with = "deserialise_expected_time"
    )]
    pub expected_time: Option<u64>,

    pub stats: TestStats,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            skip: false,
            expected_time: None,
            parser: vec!["tree-sitter".to_string(), "via-conjure".to_string()],
            rewriter: vec!["naive".to_string()],
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
            status: None,
            stats: TestStats::default(),
        }
    }
}

impl TestConfig {
    /// Empty `skip-conjure-validation` runs Conjure reference validation during accept.
    pub fn should_skip_conjure_validation(&self) -> bool {
        !self.skip_conjure_validation.is_empty()
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

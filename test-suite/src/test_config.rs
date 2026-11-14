#![allow(unused)]

use conjure_cp::settings::{
    Channelling, Heuristic, Parser, QuantifiedExpander, Rewriter, SolverFamily,
};
#[path = "../../crates/conjure-cp-cli/src/stats.rs"]
mod stats;

pub use self::stats::{
    RecordedRuleTraceStats, RecordedRunConfig, RecordedRunStats, RecordedToolStats, TestRunStats,
    read_stats_or_default, replace_config_run_stats,
};

use self::stats::{deserialise_expected_time, update_canonical_stats, write_canonical_stats};
use serde::de::{self, Visitor};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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

/// Starts a fresh stats snapshot for an exhaustive run.
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
/// Custom tests still keep their expected-time metadata in `config.toml`; exhaustive tests use
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

/// Inserts or updates the latest observed exhaustive status in a test `stats.toml`.
pub fn upsert_status_stats(path: &Path, status: &str) -> io::Result<()> {
    update_canonical_stats(path, |stats| stats.status = Some(status.to_string()))
}

/// Inserts or updates the latest observed status for one part of an exhaustive test.
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

/// Solution search limit requested by an exhaustive test.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumberOfSolutions {
    /// Search for every solution.
    All,
    /// Stop after the given number of solutions.
    Limit(i32),
    /// Do not run the solver for this exhaustive test.
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

/// Per-backend modelling and trace choices for an exhaustive fixture.
#[derive(Deserialize, Debug, Default)]
#[serde(default, deny_unknown_fields)]
pub struct SolverOptions {
    /// Override the fixture's modelling heuristic for this backend.
    #[serde(deserialize_with = "deserialize_string_or_vec")]
    pub heuristic: Vec<String>,
    /// Override the fixture's representation-sharing policy for this backend.
    #[serde(deserialize_with = "deserialize_string_or_vec")]
    pub channelling: Vec<String>,
    /// Override the fixture's trace level for this backend.
    #[serde(rename = "trace-level")]
    pub trace_level: Option<TraceLevel>,
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
    #[serde(default, rename = "solver-seed")]
    pub solver_seed: u32,
    #[serde(
        default,
        rename = "solver",
        deserialize_with = "deserialize_string_or_vec"
    )]
    pub solver: Vec<String>,

    /// Backend-specific modelling and trace overrides.
    #[serde(default, rename = "solver-options")]
    pub solver_options: BTreeMap<String, SolverOptions>,

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

    /// How much of the rule trace each run records. See [`TraceLevel`].
    #[serde(default, rename = "trace-level")]
    pub trace_level: TraceLevel,

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

/// The rule trace recorded by each exhaustive run.
#[derive(Deserialize, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TraceLevel {
    /// Disable rule trace snapshots and aggregate counts.
    None,
    /// Write the human-readable trace of every rule application and compare it with the
    /// expected snapshot.
    #[default]
    Full,
    /// Count rule applications in memory and record only the totals in `stats.toml`. Much
    /// cheaper for large fixtures, at the cost of the trace snapshot.
    Aggregate,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            skip: String::new(),
            parser: vec!["tree-sitter".to_string()],
            rewriter: vec!["optimised".to_string()],
            comprehension_expander: vec!["auto".to_string()],
            heuristic: vec!["x".to_string()],
            channelling: vec!["no".to_string()],
            seed: 0,
            solver_seed: 0,
            // How a model is expressed for a solver -- which SAT integer encoding, which Z3
            // integer theory -- is a representation choice, so the `x` heuristic above enumerates
            // those combinations rather than them being separate solvers here.
            solver: vec!["minion".to_string(), "sat".to_string(), "z3".to_string()],
            solver_options: BTreeMap::new(),
            extra_rule_sets: Vec::new(),
            minion_discrete_threshold: default_minion_discrete_threshold(),
            skip_conjure_validation: String::new(),
            number_of_solutions: NumberOfSolutions::All,
            keep_intermediate_solutions: false,
            trace_level: TraceLevel::Full,
            expected_time: None,
        }
    }
}

impl TestConfig {
    /// Complete searches share one golden across all configurations.
    pub fn uses_shared_solution_snapshot(&self) -> bool {
        matches!(self.number_of_solutions, NumberOfSolutions::All)
    }

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

    /// Resolve a backend's trace level, inheriting the fixture default when omitted.
    pub fn configured_trace_level_for(&self, solver: SolverFamily) -> TraceLevel {
        self.solver_options
            .get(&solver.as_str())
            .and_then(|options| options.trace_level)
            .unwrap_or(self.trace_level)
    }

    /// Resolve a backend's heuristic while retaining the fixture default for other backends.
    pub fn configured_heuristics_for(
        &self,
        solver: SolverFamily,
    ) -> Result<Vec<Heuristic>, String> {
        match self.solver_options.get(&solver.as_str()) {
            Some(options) if !options.heuristic.is_empty() => parse_values(&options.heuristic),
            _ => self.configured_heuristics(),
        }
    }

    /// Resolve a backend's channelling policy while retaining the fixture default elsewhere.
    pub fn configured_channelling_for(
        &self,
        solver: SolverFamily,
    ) -> Result<Vec<Channelling>, String> {
        let configured = match self.solver_options.get(&solver.as_str()) {
            Some(options) if !options.channelling.is_empty() => parse_values(&options.channelling)?,
            _ => self.configured_channelling()?,
        };
        if configured.contains(&Channelling::Yes) {
            return Err("setting 'channelling=yes' is not supported yet".to_string());
        }
        Ok(configured)
    }

    pub fn configured_solvers(&self) -> Result<Vec<SolverFamily>, String> {
        for name in self.solver_options.keys() {
            name.parse::<SolverFamily>()?;
        }
        parse_values(&self.solver)
    }

    pub fn uses_smt_solver(&self) -> bool {
        self.solver.iter().any(|solver| solver == "z3")
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
                solver_seed: 0,
                solver: solver.to_string(),
            };
            let rows = (0..2)
                .map(|model_index| RecordedRunStats {
                    parser: config.parser.clone(),
                    rewriter: config.rewriter.clone(),
                    comprehension_expander: config.comprehension_expander.clone(),
                    heuristic: config.heuristic.clone(),
                    channelling: config.channelling.clone(),
                    solver: config.solver.clone(),
                    case_name: format!("tree-sitter-auto-model-{model_index:03}"),
                    model_index,
                    choice_path: vec![model_index],
                    choices: vec![format!("choice-{model_index}")],
                    status: "ok".to_owned(),
                    translation_time: Some(translation_time),
                    solve_time: Some(3.0),
                    rule_trace: RecordedRuleTraceStats {
                        total_rule_attempts: Some(4),
                        total_rule_applications: Some(model_index as u64 + 1),
                        rules: BTreeMap::from([("rule_name".to_owned(), model_index as u64 + 1)]),
                    },
                    ..RecordedRunStats::default()
                })
                .collect();
            replace_config_run_stats(&path, &config, rows).unwrap();
        }

        let contents = fs::read_to_string(&path).unwrap();
        let document: toml::Value = toml::from_str(&contents).unwrap();
        assert!(document.get("model-runs").is_none());
        assert_eq!(document["runs"].as_array().unwrap().len(), 4);
        assert_eq!(
            document["conjure"]["wall-clock-time"].as_float(),
            Some(0.572384)
        );
        let stats = read_stats_or_default(&path).unwrap();
        assert_eq!(stats.expected_time, Some(1));
        for (index, run) in stats.runs.iter().enumerate() {
            assert_eq!(run.model_index, index % 2);
            assert_eq!(run.choice_path, vec![index % 2]);
            assert_eq!(
                run.rule_trace.total_rule_applications,
                Some((index % 2) as u64 + 1)
            );
            assert_eq!(run.rule_trace.rules["rule_name"], (index % 2) as u64 + 1);
            assert_eq!(
                run.translation_time,
                Some(if index < 2 { 1.0 } else { 2.0 })
            );
        }
    }

    #[test]
    fn replacing_a_portfolio_preserves_other_configurations_and_partial_failures() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join(STATS_FILE_NAME);
        let config = RecordedRunConfig {
            parser: "tree-sitter".to_owned(),
            solver: "minion".to_owned(),
            rewriter: "optimised".to_owned(),
            comprehension_expander: "auto".to_owned(),
            heuristic: "x".to_owned(),
            channelling: "no".to_owned(),
            seed: 42,
            solver_seed: 7,
        };
        let row = |model_index| RecordedRunStats {
            parser: config.parser.clone(),
            solver: config.solver.clone(),
            rewriter: config.rewriter.clone(),
            comprehension_expander: config.comprehension_expander.clone(),
            heuristic: config.heuristic.clone(),
            channelling: config.channelling.clone(),
            seed: config.seed,
            solver_seed: config.solver_seed,
            model_index,
            status: "ok".to_owned(),
            ..RecordedRunStats::default()
        };
        replace_config_run_stats(&path, &config, vec![row(1), row(0)]).unwrap();
        let other_config = RecordedRunConfig {
            solver_seed: 8,
            ..config.clone()
        };
        replace_config_run_stats(
            &path,
            &other_config,
            vec![RecordedRunStats {
                solver_seed: 8,
                ..row(0)
            }],
        )
        .unwrap();
        let mut failed = row(0);
        failed.status = "fail".to_owned();
        failed.translation_time = Some(1.25);
        replace_config_run_stats(&path, &config, vec![failed]).unwrap();
        let stats = read_stats_or_default(&path).unwrap();
        assert_eq!(stats.runs.len(), 2);
        assert_eq!(stats.runs[0].seed, 42);
        assert_eq!(stats.runs[0].solver_seed, 7);
        assert_eq!(stats.runs[0].status, "fail");
        assert_eq!(stats.runs[0].translation_time, Some(1.25));
        assert_eq!(stats.runs[0].solve_time, None);
        assert_eq!(stats.runs[1].solver_seed, 8);
        assert_eq!(stats.runs[1].status, "ok");
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

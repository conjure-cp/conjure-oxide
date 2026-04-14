#![allow(unused)]

use conjure_cp::settings::{Parser, QuantifiedExpander, Rewriter, SolverFamily};
use serde::Deserialize;
use std::fs;
use std::io;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use toml_edit::{DocumentMut, value};

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

fn default_true() -> bool {
    true
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

    let mut new_contents = document.to_string();
    if !new_contents.ends_with('\n') {
        new_contents.push('\n');
    }

    fs::write(path, new_contents)
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

    #[serde(default = "default_true", rename = "validate-with-conjure")]
    pub validate_with_conjure: bool,

    // Generate this test but do not run it
    pub skip: bool,

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
            validate_with_conjure: true,
        }
    }
}

impl TestConfig {
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

#![allow(unused)]

use conjure_cp::settings::{Parser, QuantifiedExpander, Rewriter, SolverFamily};
use serde::Deserialize;
use std::str::FromStr;

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

    #[serde(default = "default_true", rename = "validate-with-conjure")]
    pub validate_with_conjure: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
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
                #[cfg(feature = "smt")]
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

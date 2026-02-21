#![allow(unused)]

use conjure_cp::settings::{QuantifiedExpander, Rewriter, SatEncoding, SolverFamily};
use serde::Deserialize;
use std::env;
use std::str::FromStr;

fn split_csv(value: String) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(str::to_string)
        .collect()
}

fn ensure_kebab_case(setting: &str, value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() || !value.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
        return Err(format!(
            "setting '{setting}' value '{value}' must be kebab-case"
        ));
    }

    Ok(())
}

fn parse_values<T>(setting: &str, values: &[String]) -> Result<Vec<T>, String>
where
    T: FromStr<Err = String>,
{
    values
        .iter()
        .map(|value| {
            ensure_kebab_case(setting, value)?;
            value.parse()
        })
        .collect()
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

#[derive(Deserialize, Debug)]
#[serde(default)]
#[serde(deny_unknown_fields)]
pub struct TestConfig {
    pub extra_rewriter_asserts: Vec<String>,

    pub enable_native_parser: bool, // Stage 1a: Use the native parser instead of the legacy parser
    pub apply_rewrite_rules: bool,  // Stage 2a: Applies predefined rules to the model
    pub enable_extra_validation: bool, // Stage 2b: Runs additional validation checks

    #[serde(
        default,
        rename = "solver",
        alias = "solvers",
        deserialize_with = "deserialize_string_or_vec"
    )]
    pub solver: Vec<String>,
    #[serde(
        default,
        rename = "rewriter",
        deserialize_with = "deserialize_string_or_vec"
    )]
    pub rewriter: Vec<String>,
    #[serde(
        default,
        rename = "sat-encoding",
        alias = "sat_encoding",
        deserialize_with = "deserialize_string_or_vec"
    )]
    pub sat_encoding: Vec<String>,
    #[serde(
        default,
        rename = "quantified-expander",
        alias = "quantified_expander",
        deserialize_with = "deserialize_string_or_vec"
    )]
    pub quantified_expander: Vec<String>,

    // NOTE: legacy options kept for backwards compatibility.
    // NOTE: when adding a new solver config, make sure to update num_solvers_enabled!
    #[serde(alias = "solve-with-minion")]
    pub solve_with_minion: bool, // Stage 3a: Solves the model using Minion
    #[serde(alias = "solve-with-sat")]
    pub solve_with_sat: bool, // TODO - add stage mark
    #[serde(alias = "solve-with-smt")]
    pub solve_with_smt: bool, // TODO - add stage mark

    pub compare_solver_solutions: bool, // Stage 3b: Compares Minion and Conjure solutions
    pub validate_rule_traces: bool,     // Stage 4a: Checks rule traces against expected outputs

    #[serde(alias = "enable-morph-impl")]
    pub enable_morph_impl: bool,
    #[serde(alias = "enable-naive-impl")]
    pub enable_naive_impl: bool,
    #[serde(alias = "enable-rewriter-impl")]
    pub enable_rewriter_impl: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            extra_rewriter_asserts: vec!["vector_operators_have_partially_evaluated".into()],
            solver: vec![],
            rewriter: vec![],
            sat_encoding: vec![],
            quantified_expander: vec![],
            enable_naive_impl: true,
            solve_with_sat: false,
            solve_with_smt: false,
            enable_morph_impl: false,
            enable_rewriter_impl: true,
            enable_native_parser: true,
            apply_rewrite_rules: true,
            enable_extra_validation: false,
            solve_with_minion: true,
            compare_solver_solutions: true,
            validate_rule_traces: true,
        }
    }
}

fn env_var_override_bool(key: &str, default: bool) -> bool {
    env::var(key).ok().map(|s| s == "true").unwrap_or(default)
}

impl TestConfig {
    pub fn merge_env(self) -> Self {
        Self {
            solver: env::var("SOLVER")
                .ok()
                .map(split_csv)
                .unwrap_or(self.solver),
            rewriter: env::var("REWRITER")
                .ok()
                .map(split_csv)
                .unwrap_or(self.rewriter),
            sat_encoding: env::var("SAT_ENCODING")
                .ok()
                .map(split_csv)
                .unwrap_or(self.sat_encoding),
            quantified_expander: env::var("QUANTIFIED_EXPANDER")
                .ok()
                .map(split_csv)
                .unwrap_or(self.quantified_expander),
            enable_morph_impl: env_var_override_bool("ENABLE_MORPH_IMPL", self.enable_morph_impl),
            enable_naive_impl: env_var_override_bool("ENABLE_NAIVE_IMPL", self.enable_naive_impl),
            enable_native_parser: env_var_override_bool(
                "ENABLE_NATIVE_PARSER",
                self.enable_native_parser,
            ),
            apply_rewrite_rules: env_var_override_bool(
                "APPLY_REWRITE_RULES",
                self.apply_rewrite_rules,
            ),
            enable_extra_validation: env_var_override_bool(
                "ENABLE_EXTRA_VALIDATION",
                self.enable_extra_validation,
            ),
            solve_with_minion: env_var_override_bool("SOLVE_WITH_MINION", self.solve_with_minion),
            solve_with_sat: env_var_override_bool("SOLVE_WITH_SAT", self.solve_with_sat),
            solve_with_smt: env_var_override_bool("SOLVE_WITH_SMT", self.solve_with_smt),
            compare_solver_solutions: env_var_override_bool(
                "COMPARE_SOLVER_SOLUTIONS",
                self.compare_solver_solutions,
            ),
            validate_rule_traces: env_var_override_bool(
                "VALIDATE_RULE_TRACES",
                self.validate_rule_traces,
            ),
            enable_rewriter_impl: env_var_override_bool(
                "ENABLE_REWRITER_IMPL",
                self.enable_rewriter_impl,
            ),
            extra_rewriter_asserts: self.extra_rewriter_asserts, // Not overridden by env vars
        }
    }

    pub fn configured_solvers(&self) -> Result<Vec<SolverFamily>, String> {
        if !self.solver.is_empty() {
            return parse_values("solver", &self.solver);
        }

        let mut solvers: Vec<String> = Vec::new();
        if self.solve_with_minion {
            solvers.push("minion".to_string());
        }
        if self.solve_with_sat {
            solvers.push("sat".to_string());
        }
        if self.solve_with_smt {
            solvers.push("smt".to_string());
        }

        parse_values("solver", &solvers)
    }

    pub fn configured_rewriters(&self) -> Result<Vec<Rewriter>, String> {
        if !self.rewriter.is_empty() {
            return parse_values("rewriter", &self.rewriter);
        }

        let mut rewriters: Vec<String> = Vec::new();
        if self.enable_naive_impl {
            rewriters.push("naive".to_string());
        }
        if self.enable_morph_impl {
            rewriters.push("morph".to_string());
        }

        if rewriters.is_empty() {
            return Err("setting 'rewriter' has no values".to_string());
        }

        parse_values("rewriter", &rewriters)
    }

    pub fn configured_quantified_expanders(&self) -> Result<Vec<QuantifiedExpander>, String> {
        let values = if self.quantified_expander.is_empty() {
            vec!["native".to_string()]
        } else {
            self.quantified_expander.clone()
        };

        parse_values("quantified-expander", &values)
    }

    pub fn configured_sat_encodings(&self) -> Result<Vec<SatEncoding>, String> {
        let values = if self.sat_encoding.is_empty() {
            vec!["log".to_string()]
        } else {
            self.sat_encoding.clone()
        };

        parse_values("sat-encoding", &values)
    }

    pub fn uses_smt_solver(&self) -> bool {
        if let Ok(solvers) = self.configured_solvers() {
            #[cfg(feature = "smt")]
            {
                return solvers
                    .into_iter()
                    .any(|solver| matches!(solver, SolverFamily::Smt(_)));
            }

            #[cfg(not(feature = "smt"))]
            {
                if !solvers.is_empty() {
                    return false;
                }
            }
        }

        self.solve_with_smt
            || self
                .solver
                .iter()
                .any(|solver| solver == "smt" || solver.starts_with("smt-"))
    }

    pub fn num_solvers_enabled(&self) -> usize {
        match self.configured_solvers() {
            Ok(solvers) => solvers.len(),
            Err(_) => 0,
        }
    }
}

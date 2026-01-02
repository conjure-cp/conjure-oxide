#![allow(unused)]

use serde::Deserialize;
use std::env;

#[derive(Deserialize, Debug)]
#[serde(default)]
#[serde(deny_unknown_fields)]
pub struct TestConfig {
    pub extra_rewriter_asserts: Vec<String>,

    pub enable_native_parser: bool, // Stage 1a: Use the native parser instead of the legacy parser
    pub apply_rewrite_rules: bool,  // Stage 2a: Applies predefined rules to the model
    pub enable_extra_validation: bool, // Stage 2b: Runs additional validation checks

    // NOTE: when adding a new solver config, make sure to update num_solvers_enabled!
    pub solve_with_minion: bool, // Stage 3a: Solves the model using Minion
    pub solve_with_sat: bool,    // TODO - add stage mark
    pub solve_with_smt: bool,    // TODO - add stage mark

    pub compare_solver_solutions: bool, // Stage 3b: Compares Minion and Conjure solutions
    pub validate_rule_traces: bool,     // Stage 4a: Checks rule traces against expected outputs

    pub enable_morph_impl: bool,
    pub enable_naive_impl: bool,
    pub enable_rewriter_impl: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            extra_rewriter_asserts: vec!["vector_operators_have_partially_evaluated".into()],
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

    pub fn num_solvers_enabled(&self) -> usize {
        let mut num = 0;
        num += self.solve_with_minion as usize;
        num += self.solve_with_sat as usize;
        num += self.solve_with_smt as usize;
        num
    }
}

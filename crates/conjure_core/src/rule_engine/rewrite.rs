use std::env;
use std::fmt::Display;

use thiserror::Error;

use crate::stats::RewriterStats;
use uniplate::uniplate::Uniplate;

use crate::rule_engine::{Reduction, Rule, RuleSet};
use crate::{
    ast::Expression,
    rule_engine::resolve_rules::{
        get_rule_priorities, get_rules_vec, ResolveRulesError as ResolveError,
    },
    Model,
};

#[derive(Debug)]
struct RuleResult<'a> {
    rule: &'a Rule<'a>,
    reduction: Reduction,
}

#[derive(Debug, Error)]
pub enum RewriteError {
    ResolveRulesError(ResolveError),
}

impl Display for RewriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RewriteError::ResolveRulesError(e) => write!(f, "Error resolving rules: {}", e),
        }
    }
}

impl From<ResolveError> for RewriteError {
    fn from(error: ResolveError) -> Self {
        RewriteError::ResolveRulesError(error)
    }
}

/// Checks if the OPTIMIZATIONS environment variable is set to "0".
///
/// # Returns
/// - true if the environment variable is set to "0".
/// - false if the environment variable is not set or set to any other value.
fn optimizations_disabled() -> bool {
    match env::var("OPTIMIZATIONS") {
        Ok(val) => val == "0",
        Err(_) => false, // Assume optimizations are enabled if the variable is not set
    }
}

/// Rewrites the model by applying the rules to all constraints.
///
/// Any side-effects such as symbol table updates and top-level constraints are applied to the returned model.
///
/// # Returns
/// A copy of the model after all, if any, possible rules are applied to its constraints.
pub fn rewrite_model<'a>(
    model: &Model,
    rule_sets: &Vec<&'a RuleSet<'a>>,
) -> Result<Model, RewriteError> {
    let rule_priorities = get_rule_priorities(rule_sets)?;
    let rules = get_rules_vec(&rule_priorities);
    let mut new_model = model.clone();
    let mut stats = RewriterStats {
        is_optimization_enabled: Some(!optimizations_disabled()),
        rewriter_run_time: None,
        rewriter_rule_application_attempts: Some(0),
        rewriter_rule_applications: Some(0),
    };

    // Check if optimizations are disabled
    let apply_optimizations = !optimizations_disabled();

    let start = std::time::Instant::now();

    while let Some(step) = rewrite_iteration(
        &new_model.constraints,
        &new_model,
        &rules,
        apply_optimizations,
        &mut stats,
    ) {
        step.apply(&mut new_model); // Apply side-effects (e.g. symbol table updates)
    }
    stats.rewriter_run_time = Some(start.elapsed());
    model.context.write().unwrap().stats.add_rewriter_run(stats);
    Ok(new_model)
}

/// # Returns
/// - Some(<new_expression>) after applying the first applicable rule to `expr` or a sub-expression.
/// - None if no rule is applicable to the expression or any sub-expression.
fn rewrite_iteration<'a>(
    expression: &'a Expression,
    model: &'a Model,
    rules: &'a Vec<&'a Rule<'a>>,
    apply_optimizations: bool,
    stats: &mut RewriterStats,
) -> Option<Reduction> {
    if apply_optimizations && expression.is_clean() {
        // Skip processing this expression if it's clean
        return None;
    }

    // Mark the expression as clean - will be marked dirty if any rule is applied
    let mut expression = expression.clone();

    let rule_results = apply_all_rules(&expression, model, rules, stats);
    if let Some(new) = choose_rewrite(&rule_results) {
        // If a rule is applied, mark the expression as dirty
        return Some(new);
    }

    let mut sub = expression.children();
    for i in 0..sub.len() {
        if let Some(red) = rewrite_iteration(&sub[i], model, rules, apply_optimizations, stats) {
            sub[i] = red.new_expression;
            if let Ok(res) = expression.with_children(sub.clone()) {
                return Some(Reduction::new(res, red.new_top, red.symbols));
            }
        }
    }
    // If all children are clean, mark this expression as clean
    if apply_optimizations {
        assert!(expression.children().iter().all(|c| c.is_clean()));
        expression.set_clean(true);
        return Some(Reduction::pure(expression));
    }
    None
}

/// # Returns
/// - A list of RuleResults after applying all rules to `expression`.
/// - An empty list if no rules are applicable.
fn apply_all_rules<'a>(
    expression: &'a Expression,
    model: &'a Model,
    rules: &'a Vec<&'a Rule<'a>>,
    stats: &mut RewriterStats,
) -> Vec<RuleResult<'a>> {
    let mut results = Vec::new();
    for rule in rules {
        match rule.apply(expression, model) {
            Ok(red) => {
                log::trace!(target: "file", "Rule applicable: {:?}, to Expression: {:?}, resulting in: {:?}", rule, expression, red.new_expression);
                stats.rewriter_rule_application_attempts =
                    Some(stats.rewriter_rule_application_attempts.unwrap() + 1);
                stats.rewriter_rule_applications =
                    Some(stats.rewriter_rule_applications.unwrap() + 1);
                // Assert no clean children
                // assert!(!red.new_expression.children().iter().any(|c| c.is_clean()), "Rule that caused assertion to fail: {:?}", rule.name);
                // assert!(!red.new_expression.children().iter().any(|c| c.children().iter().any(|c| c.is_clean())));
                results.push(RuleResult {
                    rule,
                    reduction: red,
                });
            }
            Err(_) => {
                log::trace!(target: "file", "Rule attempted but not applied: {:?}, to Expression: {:?}", rule, expression);
                stats.rewriter_rule_application_attempts =
                    Some(stats.rewriter_rule_application_attempts.unwrap() + 1);
                continue;
            }
        }
    }
    results
}

/// # Returns
/// - Some(<reduction>) after applying the first rule in `results`.
/// - None if `results` is empty.
fn choose_rewrite(results: &[RuleResult]) -> Option<Reduction> {
    if results.is_empty() {
        return None;
    }
    // Return the first result for now
    Some(results[0].reduction.clone())
}

use crate::rule_engine::resolve_rules::{
    get_rule_priorities, get_rules_vec, ResolveRulesError as ResolveError,
};
use conjure_core::ast::{Expression, Model};
use conjure_core::rule::Rule;
use conjure_rules::rule_set::RuleSet;
use std::fmt::Display;
use thiserror::Error;
use uniplate::uniplate::Uniplate;

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

struct RuleResult<'a> {
    #[allow(dead_code)] // Not used yet, but will be useful to have
    rule: &'a Rule<'a>,
    new_expression: Expression,
}

/// # Returns
/// - A new expression after applying the rules to `expression` and its sub-expressions.
/// - The same expression if no rules are applicable.
pub fn rewrite<'a>(
    expression: &Expression,
    rule_sets: &Vec<&'a RuleSet<'a>>,
) -> Result<Expression, RewriteError> {
    let rule_priorities = get_rule_priorities(rule_sets)?;
    let rules = get_rules_vec(&rule_priorities);

    let mut new = expression.clone();
    while let Some(step) = rewrite_iteration(&new, &rules) {
        new = step;
    }

    Ok(new)
}

/// # Returns
/// - Some(<new_expression>) after applying the first applicable rule to `expr` or a sub-expression.
/// - None if no rule is applicable to the expression or any sub-expression.
fn rewrite_iteration<'a>(
    expression: &'a Expression,
    rules: &'a Vec<&'a Rule<'a>>,
) -> Option<Expression> {
    let rule_results = apply_all_rules(expression, rules);
    if let Some(new) = choose_rewrite(&rule_results) {
        return Some(new);
    } else {
        let mut sub = expression.children();

        for i in 0..sub.len() {
            if let Some(new) = rewrite_iteration(&sub[i], rules) {
                sub[i] = new;
                if let Ok(res) = expression.with_children(sub.clone()) {
                    return Some(res);
                }
            }
        }
    }
    None // No rules applicable to this branch of the expression
}

/// # Returns
/// - A list of RuleResults after applying all rules to `expression`.
/// - An empty list if no rules are applicable.
fn apply_all_rules<'a>(
    expression: &'a Expression,
    rules: &'a Vec<&'a Rule<'a>>,
) -> Vec<RuleResult<'a>> {
    let mut results = Vec::new();
    for rule in rules {
        match rule.apply(expression) {
            Ok(new_expression) => {
                results.push(RuleResult {
                    rule,
                    new_expression,
                });
            }
            Err(_) => continue,
        }
    }
    results
}

/// # Returns
/// - Some(<new_expression>) after applying the first rule in `results`.
/// - None if `results` is empty.
fn choose_rewrite(results: &[RuleResult]) -> Option<Expression> {
    if results.is_empty() {
        return None;
    }
    // Return the first result for now
    // println!("Applying rule: {:?}", results[0].rule);
    Some(results[0].new_expression.clone())
}

/// This rewrites the model by applying the rules to all constraints.
/// # Returns
/// - A new model with rewritten constraints.
/// - The same model if no rules are applicable.
pub fn rewrite_model<'a>(
    model: &Model,
    rule_sets: &Vec<&'a RuleSet<'a>>,
) -> Result<Model, RewriteError> {
    let mut new_model = model.clone();

    new_model.constraints = rewrite(&model.constraints, rule_sets)?;

    Ok(new_model)
}

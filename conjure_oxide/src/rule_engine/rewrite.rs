use std::fmt::Display;
use thiserror::Error;

use crate::rule_engine::resolve_rules::{
    get_rule_priorities, get_rules_vec, ResolveRulesError as ResolveError,
};
use conjure_core::ast::{Expression, Model};
use conjure_core::metadata::Metadata;
use conjure_core::rule::{Reduction, Rule};
use conjure_rules::rule_set::RuleSet;

struct RuleResult<'a> {
    #[allow(dead_code)] // Not used yet, but will be useful to have
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

    while let Some(step) = rewrite_iteration(&new_model.constraints, &new_model, &rules) {
        // println!("{:?}\n", step);
        new_model.variables.extend(step.symbols); // Add new assignments to the symbol table
                                                  // println!("After extension: {:?}\n", new_model.variables);
        if step.new_top.is_nothing() {
            new_model.constraints = step.new_expression.clone();
        } else {
            new_model.constraints = match step.new_expression {
                // Avoid creating a nested conjunction
                Expression::And(metadata, mut and) => {
                    and.push(step.new_top.clone());
                    Expression::And(metadata.clone(), and)
                }
                _ => Expression::And(
                    Metadata::new(),
                    vec![step.new_expression.clone(), step.new_top],
                ),
            };
        }
    }
    // println!("Final model: {:?}\n", new_model);
    Ok(new_model)
}

/// # Returns
/// - Some(<new_expression>) after applying the first applicable rule to `expr` or a sub-expression.
/// - None if no rule is applicable to the expression or any sub-expression.
fn rewrite_iteration<'a>(
    expression: &'a Expression,
    model: &'a Model,
    rules: &'a Vec<&'a Rule<'a>>,
) -> Option<Reduction> {
    let rule_results = apply_all_rules(expression, model, rules);
    if let Some(new) = choose_rewrite(&rule_results) {
        return Some(new);
    } else {
        match expression.sub_expressions() {
            None => {}
            Some(mut sub) => {
                for i in 0..sub.len() {
                    if let Some(red) = rewrite_iteration(sub[i], model, rules) {
                        sub[i] = &red.new_expression;
                        return Some(Reduction::new(
                            expression.clone().with_sub_expressions(sub),
                            red.new_top,
                            red.symbols,
                        ));
                    }
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
    model: &'a Model,
    rules: &'a Vec<&'a Rule<'a>>,
) -> Vec<RuleResult<'a>> {
    let mut results = Vec::new();
    for rule in rules {
        match rule.apply(expression, model) {
            Ok(red) => {
                results.push(RuleResult {
                    rule,
                    reduction: red,
                });
            }
            Err(_) => continue,
        }
    }
    results
}

/// # Returns
/// - Some(<reduction>) after applying the first rule in `results`.
/// - None if `results` is empty.
fn choose_rewrite(results: &Vec<RuleResult>) -> Option<Reduction> {
    if results.is_empty() {
        return None;
    }
    // Return the first result for now
    // println!("Applying rule: {:?}", results[0].rule);
    Some(results[0].reduction.clone())
}

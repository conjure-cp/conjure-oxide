//! Common utilities and types for rewriters.
use super::{
    resolve_rules::{ResolveRulesError, RuleData},
    Reduction,
};
use crate::pro_trace::{check_verbosity_level, Consumer, RuleTrace, VerbosityLevel};
use crate::{
    ast::{pretty::pretty_variable_declaration, Expression, SubModel},
    pro_trace::{capture_trace, TraceType},
};

use itertools::Itertools;
use std::fmt::Debug;
use thiserror::Error;

// The RuleResult struct represents the
// result of applying a rule to an expression.
#[derive(Debug, Clone)]
pub struct RuleResult<'a> {
    pub rule_data: RuleData<'a>,
    pub reduction: Reduction,
}

/// Logs the application of a rule and its effects,
/// to the main log, and the human readable traces used by the integration tester,
/// that the rule has been applied to the expression
pub fn log_rule_application(
    result: &RuleResult,
    initial_expression: &Expression,
    initial_model: &SubModel,
    consumer: &Option<Consumer>,
) {
    // extracts data from the RuleResult struct
    //  red = reduction and any constraints and variables
    let red = &result.reduction;
    let rule = result.rule_data.rule;

    // TODO include this in the trace
    //let new_top_string = pretty_vec(&red.new_top);

    // empty if no top level constraints
    let top_level_str = if red.new_top.is_empty() {
        String::new()
    } else {
        let mut exprs: Vec<String> = vec![];

        for expr in &red.new_top {
            exprs.push(format!("  {}", expr));
        }

        let exprs = exprs.iter().join("\n");

        format!("new constraints:\n{}\n", exprs)
    };

    // empty if no new variables
    // TODO: consider printing modified and removed declarations too, though removing a declaration in a rule is less likely.
    let new_variables_str = {
        let mut vars: Vec<String> = vec![];

        for var_name in red.added_symbols(&initial_model.symbols()) {
            #[allow(clippy::unwrap_used)]
            vars.push(format!(
                "  {}",
                pretty_variable_declaration(&red.symbols, &var_name).unwrap()
            ));
        }
        if vars.is_empty() {
            String::new()
        } else {
            format!("new variables:\n{}", vars.join("\n"))
        }
    };

    // logging a successful rule application
    if let Some(consumer) = consumer {
        if check_verbosity_level(consumer) != VerbosityLevel::Low {
            let rule_trace = RuleTrace {
                initial_expression: initial_expression.clone(),
                rule_name: rule.name.to_string(),
                rule_set_name: result.rule_data.rule_set.name.to_string(),
                rule_priority: result.rule_data.priority,
                transformed_expression: Some(red.new_expression.clone()),
                new_variables_str: Some(new_variables_str.to_string()),
                top_level_str: Some(top_level_str.to_string()),
            };

            capture_trace(consumer, TraceType::RuleTrace(rule_trace));
        }
    }
}

/// Represents errors that can occur during the model rewriting process.
#[derive(Debug, Error)]
pub enum RewriteError {
    #[error("Error resolving rules {0}")]
    ResolveRulesError(ResolveRulesError),
}

impl From<ResolveRulesError> for RewriteError {
    fn from(error: ResolveRulesError) -> Self {
        RewriteError::ResolveRulesError(error)
    }
}

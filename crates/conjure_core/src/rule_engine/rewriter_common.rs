//! Common utilities and types for rewriters.

use super::{resolve_rules::ResolveRulesError, Reduction, Rule};
use crate::ast::{pretty::pretty_vec, Expression};

use itertools::Itertools;
use thiserror::Error;
use tracing::{info, trace};

#[derive(Debug, Clone)]
pub struct RuleResult<'a> {
    pub rule: &'a Rule<'a>,
    pub reduction: Reduction,
}

/// Logs, to the main log, and the human readable traces used by the integration tester, that the
/// rule has been applied to the expression
pub fn log_rule_application(result: &RuleResult, initial_expression: &Expression) {
    let red = &result.reduction;
    let rule = result.rule;
    let new_top_string = pretty_vec(&red.new_top);

    info!(
        %new_top_string,
        "Rule applicable: {} ({:?}), to expression: {}, resulting in: {}",
        rule.name,
        rule.rule_sets,
        initial_expression,
        red.new_expression
    );

    trace!(
        target: "rule_engine_human",
        "{}, \n   ~~> {} ({:?}) \n{} \n\n--\n",
        initial_expression,
        rule.name,
        rule.rule_sets,
        red.new_expression,
    );

    trace!(
        target: "rule_engine",
        "; rule_applied: {} ({:?}); initial_expression: {}; tranformed_expression: {}; new_top: {}",
        rule.name,
        rule.rule_sets,
        initial_expression,
        red.new_expression,
        new_top_string,
    )
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

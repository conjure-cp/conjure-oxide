//! Common utilities and types for rewriters.

use super::{resolve_rules::ResolveRulesError, Reduction, Rule};
use crate::{
    ast::{
        pretty::{pretty_variable_declaration, pretty_vec},
        Expression,
    },
    Model,
};

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
pub fn log_rule_application(
    result: &RuleResult,
    initial_expression: &Expression,
    initial_model: &Model,
) {
    let red = &result.reduction;
    let rule = result.rule;
    let new_top_string = pretty_vec(&red.new_top);

    info!(
        %new_top_string,
        "Applying rule: {} ({:?}), to expression: {}, resulting in: {}",
        rule.name,
        rule.rule_sets,
        initial_expression,
        red.new_expression
    );

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

        for var_name in red.added_symbols(&initial_model.variables) {
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

    trace!(
        target: "rule_engine_human",
        "{}, \n   ~~> {} ({:?}) \n{} \n{}\n{}--\n",
        initial_expression,
        rule.name,
        rule.rule_sets,
        red.new_expression,
        new_variables_str,
        top_level_str
    );
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

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
    let top_level_str = if !red.new_top.is_empty() {
        let mut exprs: Vec<String> = vec![];

        for expr in &red.new_top {
            exprs.push(format!("  {}", expr));
        }

        let exprs = exprs.iter().join("\n");

        format!("with new top level expressions:\n{}\n", exprs)
    } else {
        String::new()
    };

    let mut symbol_changes: Vec<String> = vec![];

    // show symbol changes in diff-like format
    //
    // + some added decl
    // - some removed decl
    //
    // [old] x: someDomain
    // [new] x: someNewDomain

    // TODO: when we support them, print removed declarations with a - in-front of them.

    for var_name in red.added_symbols(&initial_model.variables) {
        #[allow(clippy::unwrap_used)]
        symbol_changes.push(format!(
            "  + {}",
            pretty_variable_declaration(&red.symbols, &var_name).unwrap()
        ));
    }

    for (var_name, _, _) in red.changed_symbols(&initial_model.variables) {
        #[allow(clippy::unwrap_used)]
        symbol_changes.push(format!(
            "  [old] {}",
            pretty_variable_declaration(&initial_model.variables, &var_name).unwrap()
        ));

        #[allow(clippy::unwrap_used)]
        symbol_changes.push(format!(
            "  [new] {}",
            pretty_variable_declaration(&red.symbols, &var_name).unwrap()
        ));
    }

    let symbol_changes_str = if symbol_changes.is_empty() {
        String::new()
    } else {
        format!(
            "with changed declarations:\n{}\n",
            symbol_changes.join("\n")
        )
    };

    trace!(
        target: "rule_engine_human",
        "{}, \n   ~~> {} ({:?}) \n{} \n{}\n{}--\n",
        initial_expression,
        rule.name,
        rule.rule_sets,
        red.new_expression,
        top_level_str,
        symbol_changes_str
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

//! Common utilities and types for rewriters.
use super::{
    Reduction,
    resolve_rules::{ResolveRulesError, RuleData},
    submodel_zipper::expression_ctx,
};
use crate::ast::{
    DeclarationPtr, Expression, Model, Name, SymbolTable,
    pretty::{pretty_variable_declaration, pretty_vec},
};
use crate::settings::{
    default_rule_trace_enabled, rule_trace_aggregates_enabled, rule_trace_enabled,
};

use itertools::Itertools;
use serde_json::json;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;
use thiserror::Error;
use tracing::{info, trace};

#[derive(Debug, Clone)]
pub struct RuleResult<'a> {
    pub rule_data: RuleData<'a>,
    pub reduction: Reduction,
}

pub type VariableDeclarationSnapshot = BTreeMap<Name, String>;

pub fn snapshot_variable_declarations(symbols: &SymbolTable) -> VariableDeclarationSnapshot {
    symbols
        .clone()
        .into_iter_local()
        .filter_map(|(name, _)| {
            pretty_variable_declaration(symbols, &name).map(|declaration| (name, declaration))
        })
        .collect()
}

/// Logs, to the main log, and the human readable traces used by the integration tester, that the
/// rule has been applied to the expression
pub fn log_rule_application(
    result: &RuleResult,
    initial_expression: &Expression,
    initial_symbols: &SymbolTable,
    variable_declaration_snapshots: Option<(
        &VariableDeclarationSnapshot,
        &VariableDeclarationSnapshot,
    )>,
) {
    let red = &result.reduction;
    let rule = result.rule_data.rule;

    // A reduction can only modify either constraints or clauses, not both. So the the same
    // variable is used to hold changes in both (or empty if neither are changed).
    let new_top_string = if !red.new_top.is_empty() {
        pretty_vec(&red.new_top)
    } else {
        pretty_vec(&red.new_clauses)
    };

    info!(
        %new_top_string,
        "Applying rule: {} ({:?}), to expression: {}, resulting in: {}",
        rule.name,
        rule.rule_sets,
        initial_expression,
        red.new_expression
    );

    if rule_trace_enabled() && default_rule_trace_enabled() {
        let new_constraints_str = if !red.new_top.is_empty() {
            let mut exprs: Vec<String> = vec![];
            for expr in &red.new_top {
                exprs.push(format!("  {expr}"));
            }
            let exprs = exprs.iter().join("\n");
            format!("new constraints:\n{exprs}\n")
        } else if !red.new_clauses.is_empty() {
            let mut exprs: Vec<String> = vec![];
            for clause in &red.new_clauses {
                exprs.push(format!("  {clause}"));
            }
            let exprs = exprs.iter().join("\n");
            format!("new clauses:\n{exprs}\n")
        } else {
            String::new()
        };

        let (new_variables_str, updated_variables_str) =
            if let Some((before, after)) = variable_declaration_snapshots {
                let mut new_variables = Vec::new();
                let mut updated_variables = Vec::new();

                for (name, declaration_after) in after {
                    match before.get(name) {
                        None => new_variables.push(format!("  {declaration_after}")),
                        Some(declaration_before) if declaration_before != declaration_after => {
                            updated_variables
                                .push(format!("  {declaration_before} ~~> {declaration_after}"));
                        }
                        _ => {}
                    }
                }

                let new_variables_str = if new_variables.is_empty() {
                    String::new()
                } else {
                    format!("new variables:\n{}\n", new_variables.join("\n"))
                };

                let updated_variables_str = if updated_variables.is_empty() {
                    String::new()
                } else {
                    format!("\nupdated variables:\n{}\n", updated_variables.join("\n"))
                };

                (new_variables_str, updated_variables_str)
            } else {
                // empty if no new variables
                let mut vars: Vec<String> = vec![];
                for var_name in red.added_symbols(initial_symbols) {
                    #[allow(clippy::unwrap_used)]
                    vars.push(format!(
                        "  {}",
                        pretty_variable_declaration(&red.symbols, &var_name).unwrap()
                    ));
                }
                let new_variables_str = if vars.is_empty() {
                    String::new()
                } else {
                    format!("new variables:\n{}\n", vars.join("\n"))
                };
                (new_variables_str, String::new())
            };

        trace!(
            target: "rule_engine_rule_trace",
            "{}, \n   ~~> {} ({:?})\n{}\n{}{}{}\n--\n",
            initial_expression,
            rule.name,
            rule.rule_sets,
            red.new_expression,
            new_variables_str,
            updated_variables_str,
            new_constraints_str
        );
    }

    if rule_trace_enabled() && rule_trace_aggregates_enabled() {
        trace!(
            target: "rule_engine_rule_trace_aggregates",
            rule_name = rule.name,
            "Applied rule"
        );
    }

    trace!(
        target: "rule_engine",
        "{}",
    json!({
        "rule_name": result.rule_data.rule.name,
        "rule_priority": result.rule_data.priority,
        "rule_set": {
            "name": result.rule_data.rule_set.name,
        },
        "initial_expression": serde_json::to_value(initial_expression).unwrap(),
        "transformed_expression": serde_json::to_value(&red.new_expression).unwrap()
    })

    )
}

type LettingCtxFn = Arc<dyn Fn(Expression) -> Expression>;
type ApplicableLettingRule<'a> = (
    RuleResult<'a>,
    u16,
    Expression,
    DeclarationPtr,
    LettingCtxFn,
);

pub(crate) fn try_rewrite_value_letting_once(
    model: &mut Model,
    rules_grouped: &Vec<(u16, Vec<RuleData<'_>>)>,
    prop_multiple_equally_applicable: bool,
) -> Option<()> {
    let symbols = model.symbols().clone();
    let mut results: Vec<ApplicableLettingRule<'_>> = vec![];

    'top: for (priority, rules) in rules_grouped.iter() {
        for (_, decl) in symbols.clone().into_iter_local() {
            let Some(letting_expr) = decl.as_value_letting().map(|expr| expr.clone()) else {
                continue;
            };

            for (expr, ctx) in expression_ctx(letting_expr) {
                let expr = expr.clone();
                let ctx = ctx.clone();

                for rd in rules {
                    let Ok(reduction) = (rd.rule.application)(&expr, &symbols) else {
                        continue;
                    };

                    results.push((
                        RuleResult {
                            rule_data: rd.clone(),
                            reduction,
                        },
                        *priority,
                        expr.clone(),
                        decl.clone(),
                        ctx.clone(),
                    ));
                }

                if !results.is_empty() {
                    break 'top;
                }
            }
        }
    }

    let (result, _, expr, decl, ctx) = match results.as_slice() {
        [] => return None,
        [single, ..] => single,
    };

    if prop_multiple_equally_applicable && results.len() > 1 {
        let names: Vec<_> = results
            .iter()
            .map(|(result, _, _, _, _)| result.rule_data.rule.name)
            .collect();
        panic!("Multiple equally applicable rules for value letting expression {expr}: {names:?}");
    }

    log_rule_application(result, expr, &symbols, None);

    let rewritten_expr = ctx(result.reduction.new_expression.clone());
    result.reduction.clone().apply(model);

    let mut decl = decl.clone();
    *decl
        .as_value_letting_mut()
        .expect("declaration should still be a value letting") = rewritten_expr;

    Some(())
}

/// Applies the highest-priority rule that matches the model root, if any.
///
/// Morph normally rewrites within the root expression. This hook lets root-specific rules perform
/// whole-model normalisation before the regular tree morph pass runs.
#[allow(dead_code)]
pub(crate) fn try_rewrite_root_once(
    model: &mut Model,
    rules_grouped: &Vec<(u16, Vec<RuleData<'_>>)>,
    prop_multiple_equally_applicable: bool,
) -> Option<()> {
    let symbols = model.symbols().clone();
    let root = model.root().clone();
    let mut results = Vec::new();

    'top: for (priority, rules) in rules_grouped.iter() {
        for rd in rules {
            let Ok(reduction) = (rd.rule.application)(&root, &symbols) else {
                continue;
            };

            results.push((
                RuleResult {
                    rule_data: rd.clone(),
                    reduction,
                },
                *priority,
            ));
        }

        if !results.is_empty() {
            break 'top;
        }
    }

    let (result, _) = match results.as_slice() {
        [] => return None,
        [single, ..] => single,
    };

    if prop_multiple_equally_applicable && results.len() > 1 {
        let names: Vec<_> = results
            .iter()
            .map(|(result, _)| result.rule_data.rule.name)
            .collect();
        panic!("Multiple equally applicable rules for root expression {root}: {names:?}");
    }

    log_rule_application(result, &root, &symbols, None);
    result.reduction.clone().apply(model);
    model.replace_root(result.reduction.new_expression.clone());

    Some(())
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

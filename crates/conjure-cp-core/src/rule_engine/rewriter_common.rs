//! Common utilities and types for rewriters.
use super::{
    RuleEffect,
    expression_zipper::expression_ctx,
    resolve_rules::{ResolveRulesError, RuleData},
};
use crate::ast::{
    DeclarationPtr, Expression, Model, Name, SymbolTable,
    pretty::{pretty_variable_declaration, pretty_vec},
};
use crate::settings::{
    Heuristic, default_rule_trace_enabled, heuristic, next_heuristic_all_index,
    next_heuristic_interactive_index, next_heuristic_random_index, rule_trace_aggregates_enabled,
    rule_trace_enabled,
};

use itertools::Itertools;
use serde_json::json;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;
use thiserror::Error;
use tracing::{info, trace};
use uniplate::Uniplate;

#[derive(Debug, Clone)]
pub struct RuleResult<'a> {
    pub rule_data: RuleData<'a>,
    pub effect: RuleEffect,
}

fn expression_ast_depth(expression: &Expression) -> usize {
    1 + expression
        .children()
        .iter()
        .map(expression_ast_depth)
        .max()
        .unwrap_or(0)
}

fn effect_ast_depth(effect: &RuleEffect) -> usize {
    std::iter::once(&effect.new_expression)
        .chain(effect.new_top.iter())
        .map(expression_ast_depth)
        .max()
        .unwrap_or(0)
}

fn rule_result_compact_depth(result: &RuleResult<'_>) -> usize {
    // Deferred effects deliberately cannot be materialised speculatively. Prefer a concrete
    // effect when compactness can actually be measured.
    result
        .effect
        .is_deferred()
        .then_some(usize::MAX)
        .unwrap_or_else(|| effect_ast_depth(&result.effect))
}

/// Chooses between rules applicable at the same priority and focus.
pub(crate) fn choose_rule_result_index<'a, 'b>(
    results: impl ExactSizeIterator<Item = &'b RuleResult<'a>>,
) -> usize
where
    'a: 'b,
{
    let results: Vec<_> = results.collect();
    debug_assert!(!results.is_empty());
    if results.len() == 1 {
        return 0;
    }
    match heuristic() {
        Heuristic::First => 0,
        Heuristic::Random => next_heuristic_random_index(results.len()),
        Heuristic::Compact => results
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                rule_result_compact_depth(left)
                    .cmp(&rule_result_compact_depth(right))
                    .then_with(|| left.rule_data.rule.name.cmp(right.rule_data.rule.name))
            })
            .map(|(index, _)| index)
            .unwrap_or(0),
        Heuristic::Interactive => {
            let names: Vec<_> = results
                .iter()
                .map(|result| result.rule_data.rule.name)
                .collect();
            next_heuristic_interactive_index(&names)
        }
        Heuristic::All => {
            let names: Vec<_> = results
                .iter()
                .map(|result| result.rule_data.rule.name)
                .collect();
            next_heuristic_all_index(&names)
        }
    }
}

pub type VariableDeclarationSnapshot = BTreeMap<Name, String>;

/// Pretty-prints every local variable declaration for rule-trace before/after diffs.
///
/// This is intentionally expensive (string formatting of domains). Callers must only invoke it
/// when the default human-readable rule trace needs declaration diffs.
pub fn snapshot_variable_declarations(symbols: &SymbolTable) -> VariableDeclarationSnapshot {
    symbols
        .clone()
        .into_iter_local()
        .filter_map(|(name, _)| {
            pretty_variable_declaration(symbols, &name).map(|declaration| (name, declaration))
        })
        .collect()
}

/// Captures a Root declaration snapshot only when the default rule-trace sink is enabled.
///
/// Failed rule attempts and aggregate/verbose-only tracing must not pay for pretty-printing.
/// The effect has not been applied yet, so a post-success call is still a valid "before" snapshot.
pub fn root_variable_snapshot_for_default_trace(
    expr: &Expression,
    symbols: &SymbolTable,
) -> Option<VariableDeclarationSnapshot> {
    if !matches!(expr, Expression::Root(_, _)) {
        return None;
    }
    if !(rule_trace_enabled() && default_rule_trace_enabled()) {
        return None;
    }
    Some(snapshot_variable_declarations(symbols))
}

/// Snapshots variable declarations after applying a rule's symbol-table changes.
pub fn snapshot_symbols_after_effect(
    symbols: &SymbolTable,
    effect: &RuleEffect,
) -> VariableDeclarationSnapshot {
    let mut merged = symbols.clone();
    effect.preview_declaration_updates(&mut merged);
    merged.extend(effect.symbols.clone());
    snapshot_variable_declarations(&merged)
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
    let red = &result.effect;
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
            "{}\n   ~~> {} ({:?})\n{}\n{}{}{}\n--\n",
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
) -> Option<Name> {
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
                    let Ok(effect) = (rd.rule.application)(&expr, &symbols) else {
                        continue;
                    };

                    results.push((
                        RuleResult {
                            rule_data: rd.clone(),
                            effect,
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

    if prop_multiple_equally_applicable && results.len() > 1 {
        let expr = &results[0].2;
        let names: Vec<_> = results
            .iter()
            .map(|(result, _, _, _, _)| result.rule_data.rule.name)
            .collect();
        panic!("Multiple equally applicable rules for value letting expression {expr}: {names:?}");
    }

    if results.is_empty() {
        return None;
    }
    let selected = choose_rule_result_index(results.iter().map(|(result, ..)| result));
    results.swap(0, selected);
    let (result, _, expr, decl, ctx) = &results[0];

    let effect = result.effect.materialise(&symbols);
    let result = RuleResult {
        rule_data: result.rule_data.clone(),
        effect,
    };

    log_rule_application(&result, expr, &symbols, None);

    let rewritten_expr = ctx(result.effect.new_expression.clone());
    result.effect.apply(model);

    let mut decl = decl.clone();
    *decl
        .as_value_letting_mut()
        .expect("declaration should still be a value letting") = rewritten_expr;

    model.symbols_mut().refresh_local_binding_hashes();

    Some(decl.name().clone())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Literal, Metadata, Moo};
    use crate::rule_engine::{ApplicationError, Rule, RuleSet};
    use crate::settings::{
        Heuristic, begin_heuristic_all_choices, heuristic, heuristic_all_choices,
        set_default_rule_trace_enabled, set_heuristic, set_heuristic_responses,
        set_rule_trace_enabled,
    };

    fn test_rule_set_applies(_: &crate::settings::SolverFamily) -> bool {
        true
    }

    fn never_apply(_: &Expression, _: &SymbolTable) -> Result<RuleEffect, ApplicationError> {
        Err(ApplicationError::RuleNotApplicable)
    }

    static TEST_RULE_SET: RuleSet<'static> =
        RuleSet::new("heuristic-test", &[], test_rule_set_applies);
    static FIRST_RULE: Rule<'static> =
        Rule::new("first-rule", never_apply, &[("heuristic-test", 1)]);
    static SECOND_RULE: Rule<'static> =
        Rule::new("second-rule", never_apply, &[("heuristic-test", 1)]);

    fn heuristic_result(
        rule: &'static Rule<'static>,
        expression: Expression,
    ) -> RuleResult<'static> {
        RuleResult {
            rule_data: RuleData {
                rule,
                priority: 1,
                rule_set: &TEST_RULE_SET,
            },
            effect: RuleEffect::pure(expression),
        }
    }

    struct HeuristicGuard(Heuristic);

    impl HeuristicGuard {
        fn set(value: Heuristic) -> Self {
            let guard = Self(heuristic());
            set_heuristic(value);
            guard
        }
    }

    impl Drop for HeuristicGuard {
        fn drop(&mut self) {
            set_heuristic(self.0);
        }
    }

    /// Restores process-local rule-trace flags after a test mutates them.
    struct RuleTraceFlagGuard {
        rule_trace: bool,
        default_rule_trace: bool,
    }

    impl RuleTraceFlagGuard {
        /// Saves the current flags and applies `rule_trace` / `default_rule_trace`.
        fn set(rule_trace: bool, default_rule_trace: bool) -> Self {
            let guard = Self {
                rule_trace: rule_trace_enabled(),
                default_rule_trace: default_rule_trace_enabled(),
            };
            set_rule_trace_enabled(rule_trace);
            set_default_rule_trace_enabled(default_rule_trace);
            guard
        }
    }

    impl Drop for RuleTraceFlagGuard {
        fn drop(&mut self) {
            set_rule_trace_enabled(self.rule_trace);
            set_default_rule_trace_enabled(self.default_rule_trace);
        }
    }

    #[test]
    fn root_snapshot_skipped_when_default_rule_trace_disabled() {
        let _guard = RuleTraceFlagGuard::set(true, false);
        let root = Expression::Root(Metadata::new(), vec![Literal::Bool(true).into()]);
        let symbols = SymbolTable::new();
        assert!(root_variable_snapshot_for_default_trace(&root, &symbols).is_none());
    }

    #[test]
    fn root_snapshot_taken_only_for_root_when_default_rule_trace_enabled() {
        let _guard = RuleTraceFlagGuard::set(true, true);
        let root = Expression::Root(Metadata::new(), vec![Literal::Bool(true).into()]);
        let leaf: Expression = Literal::Bool(true).into();
        let symbols = SymbolTable::new();
        assert!(root_variable_snapshot_for_default_trace(&root, &symbols).is_some());
        assert!(root_variable_snapshot_for_default_trace(&leaf, &symbols).is_none());
    }

    #[test]
    fn compact_heuristic_chooses_shallower_rule_effect() {
        let _guard = HeuristicGuard::set(Heuristic::Compact);
        let leaf: Expression = Literal::Bool(true).into();
        let deep = Expression::Not(Metadata::new(), Moo::new(leaf.clone()));
        let results = [
            heuristic_result(&FIRST_RULE, deep),
            heuristic_result(&SECOND_RULE, leaf),
        ];
        assert_eq!(choose_rule_result_index(results.iter()), 1);
    }

    #[test]
    fn all_heuristic_records_equally_applicable_rules() {
        let _guard = HeuristicGuard::set(Heuristic::All);
        begin_heuristic_all_choices(vec![1]);
        let results = [
            heuristic_result(&FIRST_RULE, Literal::Bool(true).into()),
            heuristic_result(&SECOND_RULE, Literal::Bool(false).into()),
        ];
        assert_eq!(choose_rule_result_index(results.iter()), 1);
        assert_eq!(
            heuristic_all_choices()[0].options,
            vec!["first-rule".to_string(), "second-rule".to_string()]
        );
    }

    #[test]
    fn interactive_heuristic_uses_one_based_responses() {
        let _guard = HeuristicGuard::set(Heuristic::Interactive);
        set_heuristic_responses(vec![2]);
        let results = [
            heuristic_result(&FIRST_RULE, Literal::Bool(true).into()),
            heuristic_result(&SECOND_RULE, Literal::Bool(false).into()),
        ];
        assert_eq!(choose_rule_result_index(results.iter()), 1);
    }
}

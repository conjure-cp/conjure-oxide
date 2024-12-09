use std::sync::Arc;

use super::{RewriteError, RuleSet};
use crate::{
    ast::{pretty::pretty_vec, Expression as Expr},
    bug,
    rule_engine::{
        get_rule_priorities,
        rewriter_common::{log_rule_application, RuleResult},
        Rule,
    },
    Model,
};
use std::collections::{BTreeMap, HashSet};
use uniplate::Biplate;

/// A naive, exhaustive rewriter.
///
/// **This rewriter should not be used in production, and is intended as a development tool.**
///
/// The goal of this rewriter is to model the correct rule application order. To this end, it uses
/// the simplest implementation possible, disregarding performance.
///
/// **Rule application order:** apply the highest priority rule possible anywhere in the tree,
/// favouring larger expressions as a tie-breaker.

pub fn rewrite_naive<'a>(
    model: &Model,
    rule_sets: &Vec<&'a RuleSet<'a>>,
) -> Result<Model, RewriteError> {
    // At each iteration, rules are checked against all expressions in order of priority until one
    // is applicable. This is done until no more rules can be applied to any expression.

    let priorities =
        get_rule_priorities(rule_sets).unwrap_or_else(|_| bug!("get_rule_priorities() failed!"));

    // Invert the map: group rules by their priority.
    // BTreeMap because it maintains keys in sorted order
    let mut grouped: BTreeMap<u16, HashSet<&'a Rule<'a>>> = BTreeMap::new();

    for (rule, priority) in priorities {
        grouped
            .entry(priority)
            .or_insert_with(HashSet::new)
            .insert(rule);
    }

    // `grouped` now holds rules organised by ascending priority
    // If needed, you can convert this into a Vec of sets:
    let rules_by_priority: Vec<(u16, HashSet<&'a Rule<'a>>)> = grouped.into_iter().collect();

    type CtxFn = Arc<dyn Fn(Expr) -> Vec<Expr>>;

    let mut model = model.clone();

    loop {
        // List of applicable rules for this pass:
        //
        // (reduction, rule name, priority, original expression, context function)
        //
        // Each rule in this list should be at the same priority level
        let mut results: Vec<(RuleResult<'_>, u16, Expr, CtxFn)> = vec![];

        // Iterate over priority levels in descending order
        for (priority, rule_set) in rules_by_priority.iter().rev() {
            for (expr, ctx) in Biplate::<Expr>::contexts_bi(&model.get_constraints_vec()) {
                // Clone expr and ctx so they can be reused
                let expr = expr.clone();
                let ctx = ctx.clone();
                for rule in rule_set {
                    match (rule.application)(&expr, &model) {
                        Ok(red) => {
                            // Collect applicable rules
                            results.push((
                                RuleResult {
                                    rule: rule,
                                    reduction: red,
                                },
                                *priority,
                                expr.clone(),
                                ctx.clone(),
                            ));
                        }
                        Err(_) => {
                            tracing::trace!(
                                "Rule attempted but not applied: {} (priority {}), to expression: {}",
                                rule.name,
                                priority,
                                expr
                            );
                        }
                    }
                }
                // If any results were found at the current priority level, stop checking lower priorities
                if !results.is_empty() {
                    break;
                }
            }
        }

        match results.as_slice() {
            [] => {
                break;
            }
            [(result, priority, expr, ctx)] => {
                // Extract the single applicable rule and apply it
                tracing::info!(
                    new_top = %pretty_vec(&result.reduction.new_top),
                    "Applying rule: {} (priority {}), to expression: {}, resulting in: {}",
                    result.rule.name,
                    priority,
                    expr,
                    result.reduction.new_expression
                );
                // let (result, expr, ctx) = results[0].clone();

                log_rule_application(result, &expr);

                // Replace expr with new_expression
                model.set_constraints(ctx(result.reduction.new_expression.clone()));

                // Apply new symbols and top level
                result.reduction.clone().apply(&mut model);
            }
            _ => {
                let names: Vec<_> = results
                    .iter()
                    .map(|(result, _, _, _)| result.rule.name)
                    .collect();

                // Extract the expression from the first result
                let expr = results[0].2.clone();

                // bug!("Multiple equally applicable rules for {expr}: {names:#?}");

                // TODO, debugging code, remove before merging
                // TODO: write a separate test which generates this for a given backend solver and tests it using generated/expected style. Good for documentation too.
                // Current output:
                // Priority 9001:
                // - apply_eval_constant
                // Priority 9000:
                // - partial_evaluator
                // Priority 8900:
                // - bubble_up
                // - expand_bubble
                // Priority 8800:
                // - remove_empty_expression
                // - remove_unit_vector_and
                // - remove_unit_vector_sum
                // - negated_eq_to_neq
                // - negated_neq_to_eq
                // - remove_unit_vector_or
                // Priority 8400:
                // - distribute_negation_over_sum
                // - distribute_not_over_or
                // - minus_to_sum
                // - distribute_not_over_and
                // - remove_double_negation
                // - distribute_or_over_and
                // - normalise_associative_commutative
                // - elmininate_double_negation
                // Priority 6000:
                // - mod_to_bubble
                // - div_to_bubble
                // Priority 4400:
                // - flatten_vecop
                // - flatten_eq
                // - sum_leq_to_sumleq
                // - sum_eq_to_sumeq
                // - x_leq_y_plus_k_to_ineq
                // - flatten_sum_geq
                // - flatten_binop
                // - sumeq_to_minion
                // Priority 4200:
                // - introduce_modeq
                // - introduce_diveq
                // Priority 4100:
                // - leq_to_ineq
                // - gt_to_ineq
                // - geq_to_ineq
                // - lt_to_ineq
                // - not_literal_to_wliteral
                // Priority 4090:
                // - not_constraint_to_reify
                // Priority 2000:
                // - min_to_var
                // Priority 100:
                // - max_to_var

                // Construct a single string to display the names of the rules grouped by priority
                let mut rules_by_priority_string = String::new();
                rules_by_priority_string.push_str("Rules grouped by priority:\n");
                for (priority, rule_set) in rules_by_priority.iter().rev() {
                    rules_by_priority_string.push_str(&format!("Priority {}:\n", priority));
                    for rule in rule_set {
                        rules_by_priority_string.push_str(&format!("  - {}\n", rule.name));
                    }
                }
                bug!("Multiple equally applicable rules for {expr}: {names:#?}\n\n{rules_by_priority_string}");
            }
        }
    }

    Ok(model)
}

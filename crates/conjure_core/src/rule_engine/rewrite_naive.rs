use std::sync::Arc;

use uniplate::Biplate;

use super::{RewriteError, RuleSet};
use crate::{
    ast::{pretty::pretty_vec, Expression as Expr},
    bug,
    rule_engine::{get_rule_priorities, get_rules_vec, Reduction},
    Model,
};

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

    // rules sorted by priority
    let rules = get_rules_vec(
        &get_rule_priorities(rule_sets).unwrap_or_else(|_| bug!("get_rule_priorities() failed!")),
    );

    type CtxFn = Arc<dyn Fn(Expr) -> Vec<Expr>>;

    // List of applicable rules for this pass:
    //
    // (reduction,rule name, priority, original expression, context function)
    //
    // Each rule in this list should be at the same priority level
    let mut results: Vec<(Reduction, String, u16, Expr, CtxFn)>;

    let mut model = model.clone();
    let mut highest_applicable_rule_priority = 0;

    loop {
        results = vec![];

        // already found a rule of x priority that's applicable, do not bother trying ones that are
        // lower.
        highest_applicable_rule_priority = 0;

        for rule in &rules {
            let rule_priority = rule.rule_sets[0].1;

            if rule_priority < highest_applicable_rule_priority {
                break;
            }

            for (expr, ctx) in Biplate::<Expr>::contexts_bi(&model.get_constraints_vec()) {
                let Ok(red) = (rule.application)(&expr, &model) else {
                    tracing::trace!(
                        "Rule attempted but not applied: {} ({:?}), to expression: {}",
                        rule.name,
                        rule.rule_sets,
                        expr
                    );
                    continue;
                };

                highest_applicable_rule_priority = rule_priority;
                results.push((
                    red,
                    rule.name.into(),
                    highest_applicable_rule_priority,
                    expr,
                    ctx,
                ));
            }
        }

        // have we found a valid reduction?
        if results.is_empty() {
            break;
        };

        let (red, name, priority, expr, ctx) = results[0].clone();

        // are there any equally applicable rules?
        let mut also_applicable: Vec<_> = results[1..]
            .iter()
            .filter(|(_, _, p, e, _)| *e == expr && *p == priority)
            .collect();

        if !also_applicable.is_empty() {
            also_applicable.push(&results[0]);
            let names: Vec<_> = also_applicable.iter().map(|x| x.2).collect();
            let expr = expr.clone();
            bug!("Multiple equally applicable rules for {expr}: {names:#?}");
        }

        tracing::info!(
            new_top = %pretty_vec(&red.new_top),
            "Applying rule: {} (priority {}), to expression: {}, resulting in: {}",
            name,
            priority,
            expr,
            &red.new_expression
        );

        // replace expr with new_expression
        model.set_constraints(ctx(red.new_expression.clone()));

        // apply new symbols and top level
        red.apply(&mut model);
    }
    Ok(model)
}

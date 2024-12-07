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

    // result of the pass, including some logging information
    // reduction,rule name, priority, original expression, context fn
    let mut result: Option<(Reduction, String, u16, Expr, CtxFn)>;

    let mut model = model.clone();

    loop {
        result = None;

        'rules: for rule in &rules {
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

                result = Some((red, rule.name.into(), rule.rule_sets[0].1, expr, ctx));
                break 'rules;
            }
        }

        // have we found a valid reduction?
        let Some((red, name, priority, expr, ctx)) = result else {
            break;
        };

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

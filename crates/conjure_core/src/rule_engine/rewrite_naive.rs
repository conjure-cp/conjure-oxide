use std::sync::Arc;

use uniplate::Biplate;

use super::{RewriteError, RuleSet};
use crate::{
    ast::Expression as Expr,
    bug,
    rule_engine::{
        get_rule_priorities, get_rules_vec,
        rewriter_common::{log_rule_application, RuleResult},
    },
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
    // (ruleresult, original expression, context function)
    //
    // Each rule in this list should be at the same priority level
    let mut results: Vec<(RuleResult, Expr, CtxFn)>;

    let mut model = model.clone();
    let mut highest_applicable_rule_priority;

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
                let Ok(reduction) = (rule.application)(&expr, &model) else {
                    tracing::trace!(
                        "Rule attempted but not applied: {} ({:?}), to expression: {}",
                        rule.name,
                        rule.rule_sets,
                        expr
                    );
                    continue;
                };

                highest_applicable_rule_priority = rule_priority;
                results.push((RuleResult { rule, reduction }, expr, ctx));
            }
        }

        // have we found a valid reduction?
        if results.is_empty() {
            break;
        };

        let (result, expr, ctx) = results[0].clone();

        // are there any equally applicable rules?
        let mut also_applicable: Vec<_> = results[1..]
            .iter()
            .filter(|(r, e, _)| *e == expr && r.rule.rule_sets[0].1 == result.rule.rule_sets[0].1)
            .collect();

        if !also_applicable.is_empty() {
            also_applicable.push(&results[0]);
            let names: Vec<_> = also_applicable.iter().map(|x| x.0.rule.name).collect();
            let expr = &expr;
            bug!("Multiple equally applicable rules for {expr}: {names:#?}");
        }

        log_rule_application(&result, &expr);

        // replace expr with new_expression
        model.set_constraints(ctx(result.reduction.new_expression.clone()));

        // apply new symbols and top level
        result.reduction.apply(&mut model);
    }
    Ok(model)
}

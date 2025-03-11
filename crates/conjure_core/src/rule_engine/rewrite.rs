use std::env;

use itertools::Itertools;
use uniplate::Uniplate;

use crate::ast::{Expression, ReturnType};
use crate::bug;
use crate::rule_engine::{get_rules, Reduction, RuleSet};
use crate::stats::RewriterStats;
use crate::Model;

use super::resolve_rules::RuleData;
use super::rewriter_common::{log_rule_application, RewriteError, RuleResult};

/// Checks if the OPTIMIZATIONS environment variable is set to "1".
fn optimizations_enabled() -> bool {
    match env::var("OPTIMIZATIONS") {
        Ok(val) => val == "1",
        Err(_) => false, // Assume optimizations are disabled if the environment variable is not set
    }
}

/// Rewrites the given model by applying a set of rules to all its constraints, until no more rules can be applied.
///
/// Rules are applied in order of priority (from highest to lowest)
/// Rules can:
/// - Apply transformations to the constraints in the model (or their sub-expressions)
/// - Add new constraints to the model
/// - Modify the symbol table (e.g. add new variables)
///
/// # Parameters
/// - `model`: A reference to the [`Model`] to be rewritten.
/// - `rule_sets`: A vector of references to [`RuleSet`]s to be applied.
///
/// Each `RuleSet` contains a map of rules to priorities.
///
/// # Returns
/// - `Ok(Model)`: If successful, it returns a modified copy of the [`Model`]
/// - `Err(RewriteError)`: If an error occurs during rule application (e.g., invalid rules)
///
/// # Side-Effects
/// - Rules can apply side-effects to the model (e.g. adding new constraints or variables).
///   The original model is cloned and a modified copy is returned.
/// - Rule engine statistics (e.g. number of rule applications, run time) are collected and stored in the new model's context.
///
/// # Example
/// - Using `rewrite_model` with the constraint `(a + min(x, y)) = b`
///
///   Original model:
///   ```text
///   model: {
///     constraints: [(a + min(x, y) + 42 - 10) = b],
///     symbols: [a, b, x, y]
///   }
///   rule_sets: [{
///       name: "MyRuleSet",
///       rules: [
///         min_to_var: 10,
///         const_eval: 20
///       ]
///     }]
///   ```
///
///   Rules:
///   - `min_to_var`: min([a, b]) ~> c ; c <= a & c <= b & (c = a \/ c = b)
///   - `const_eval`: c1 + c2 ~> (c1 + c2) ; c1, c2 are constants
///
///   Result:
///   ```text
///   model: {
///     constraints: [
///       (a + aux + 32) = b,
///       aux <= x,
///       aux <= y,
///       aux = x \/ aux = y
///     ],
///     symbols: [a, b, x, y, aux]
///   }
///   ```
///
///   Process:
///   1. We traverse the expression tree until a rule can be applied.
///   2. If multiple rules can be applied to the same expression, the higher priority one goes first.
///      In this case, `const_eval` is applied before `min_to_var`.
///   3. The rule `min_to_var` adds a new variable `aux` and new constraints to the model.
///   4. When no more rules can be applied, the resulting model is returned.
///
///   Details for this process can be found in [`rewrite_iteration`] documentation.
///
/// # Performance Considerations
/// - We recursively traverse the tree multiple times to check if any rules can be applied.
/// - Expressions are cloned on each rule application
///
/// This can be expensive for large models
///
/// # Panics
/// - This function may panic if the model's context is unavailable or if there is an issue with locking the context.
///
/// # See Also
/// - [`get_rules`]: Resolves the rules from the provided rule sets and sorts them by priority.
/// - [`rewrite_iteration`]: Executes a single iteration of rewriting the model using the specified rules.
pub fn rewrite_model<'a>(
    model: &Model,
    rule_sets: &Vec<&'a RuleSet<'a>>,
) -> Result<Model, RewriteError> {
    let rules = get_rules(rule_sets)?.into_iter().collect();
    let mut new_model = model.clone();
    let mut stats = RewriterStats {
        is_optimization_enabled: Some(optimizations_enabled()),
        rewriter_run_time: None,
        rewriter_rule_application_attempts: Some(0),
        rewriter_rule_applications: Some(0),
    };

    // Check if optimizations are enabled
    let apply_optimizations = optimizations_enabled();

    let start = std::time::Instant::now();

    //the while loop is exited when None is returned implying the sub-expression is clean
    let mut i: usize = 0;
    while i < new_model.as_submodel().constraints().len() {
        while let Some(step) = rewrite_iteration(
            &new_model.as_submodel().constraints()[i],
            &new_model,
            &rules,
            apply_optimizations,
            &mut stats,
        ) {
            debug_assert!(is_vec_bool(&step.new_top)); // All new_top expressions should be boolean
            new_model.as_submodel_mut().constraints_mut()[i] = step.new_expression.clone();
            step.apply(new_model.as_submodel_mut()); // Apply side-effects (e.g., symbol table updates)
        }

        // If new constraints are added, continue processing them in the next iterations.
        i += 1;
    }

    stats.rewriter_run_time = Some(start.elapsed());
    model.context.write().unwrap().stats.add_rewriter_run(stats);
    Ok(new_model)
}

/// Checks if all expressions in `Vec<Expr>` are booleans.
/// All top-level constraints in a model should be boolean expressions.
fn is_vec_bool(exprs: &[Expression]) -> bool {
    exprs
        .iter()
        .all(|expr| expr.return_type() == Some(ReturnType::Bool))
}

/// Attempts to apply a set of rules to the given expression and its sub-expressions in the model.
///
/// 1. Checks if the expression is "clean" (all possible rules have been applied).
/// 2. Tries to apply rules to the top-level expression, in oprder of priority.
/// 3. If no rules can be applied to the top-level expression, recurses into its sub-expressions.
///
/// When a successful rule application is found, immediately returns a `Reduction` and stops.
/// The `Reduction` contains the new expression and any side-effects (e.g., new constraints, variables).
/// If no rule applications are possible in this expression tree, returns `None`.
///
/// # Parameters
/// - `expression`: The [`Expression`] to be rewritten.
/// - `model`: The root [`Model`] for access to the context and symbol table.
/// - `rules`: A max-heap of [`RuleData`] containing rules, priorities, and metadata. Ordered by rule priority.
/// - `apply_optimizations`: If `true`, skip already "clean" expressions to avoid redundant work.
/// - `stats`: A mutable reference to [`RewriterStats`] to collect statistics
///
/// # Returns
/// - `Some(<Reduction>)`: If a rule is successfully applied to the expression or any of its sub-expressions.
///                        Contains the new expression and any side-effects to apply to the model.
/// - `None`: If no rule is applicable to the expression or any of its sub-expressions.
///
/// # Example
///
/// - Rewriting the expression `a + min(x, y)`:
///
///   Input:
///   ```text
///   expression: a + min(x, y)
///   rules: [min_to_var]
///   model: {
///     constraints: [(a + min(x, y)) = b],
///     symbols: [a, b, x, y]
///   }
///   apply_optimizations: true
///   ```
///
///   Process:
///   1. Initially, the expression is dirty, so we proceed with the rewrite.
///   2. No rules can be applied to the top-level expression `a + min(x, y)`.
///      Try its children: `a` and `min(x, y)`.
///   3. No rules can be applied to `a`. Mark it as clean and return None.
///   4. The rule `min_to_var` can be applied to `min(x, y)`. Return the `Reduction`.
///      ```text
///      Reduction {
///        new_expression: aux,
///        new_top: [aux <= x, aux <= y, aux = x \/ aux = y],
///        symbols: [a, b, x, y, aux]
///      }
///      ```
///   5. Update the parent expression `a + min(x, y)` with the new child `a + aux`.
///      Add new constraints and variables to the model.
///   6. No more rules can be applied to this expression. Mark it as clean and return a pure `Reduction`.
fn rewrite_iteration(
    expression: &Expression,
    model: &Model,
    rules: &Vec<RuleData<'_>>,
    apply_optimizations: bool,
    stats: &mut RewriterStats,
) -> Option<Reduction> {
    if apply_optimizations && expression.is_clean() {
        // Skip processing this expression if it's clean
        return None;
    }

    // Mark the expression as clean - will be marked dirty if any rule is applied
    let mut expression = expression.clone();

    let rule_results = apply_all_rules(&expression, model, rules, stats);
    if let Some(result) = choose_rewrite(&rule_results, &expression) {
        // If a rule is applied, mark the expression as dirty
        log_rule_application(&result, &expression, model.as_submodel());
        return Some(result.reduction);
    }

    let mut sub = expression.children();
    for i in 0..sub.len() {
        if let Some(red) = rewrite_iteration(&sub[i], model, rules, apply_optimizations, stats) {
            sub[i] = red.new_expression;
            let res = expression.with_children(sub.clone());
            return Some(Reduction::new(res, red.new_top, red.symbols));
        }
    }
    // If all children are clean, mark this expression as clean
    if apply_optimizations {
        assert!(expression.children().iter().all(|c| c.is_clean()));
        expression.set_clean(true);
        return Some(Reduction::pure(expression));
    }
    None
}

/// Tries to apply rules to an expression and returns a list of successful applications.
///
/// The expression or model is NOT modified directly.
/// We create a list of `RuleResult`s containing the reductions and pass it to `choose_rewrite` to select one to apply.
///
/// # Parameters
/// - `expression`: A reference to the [`Expression`] to evaluate.
/// - `model`: A reference to the [`Model`] for access to the symbol table and context.
/// - `rules`: A vector of references to [`Rule`]s to try.
/// - `stats`: A mutable reference to [`RewriterStats`] used to track the number of rule applications and other statistics.
///
/// # Returns
/// - A `Vec<RuleResult>` containing all successful rule applications to the expression.
///   Each `RuleResult` contains the rule that was applied and the resulting `Reduction`.
///
/// # Side-Effects
/// - The function updates the provided `stats` with the number of rule application attempts and successful applications.
/// - Debug or trace logging may be performed to track which rules were applicable or not for a given expression.
///
/// # Example
/// let applicable_rules = apply_all_rules(&expr, &model, &rules, &mut stats);
/// if !applicable_rules.is_empty() {
///     for result in applicable_rules {
///         println!("Rule applied: {:?}", result.rule_data.rule);
///     }
/// }
///
/// ## Note
/// - Rules are applied only to the given expression, not its children.
///
/// # See Also
/// - [`choose_rewrite`]: Chooses a single reduction from the rule results provided by `apply_all_rules`.
fn apply_all_rules<'a>(
    expression: &Expression,
    model: &Model,
    rules: &Vec<RuleData<'a>>,
    stats: &mut RewriterStats,
) -> Vec<RuleResult<'a>> {
    let mut results = Vec::new();
    for rule_data in rules {
        match rule_data
            .rule
            .apply(expression, &model.as_submodel().symbols())
        {
            Ok(red) => {
                stats.rewriter_rule_application_attempts =
                    Some(stats.rewriter_rule_application_attempts.unwrap() + 1);
                stats.rewriter_rule_applications =
                    Some(stats.rewriter_rule_applications.unwrap() + 1);
                // Assert no clean children
                // assert!(!red.new_expression.children().iter().any(|c| c.is_clean()), "Rule that caused assertion to fail: {:?}", rule.name);
                // assert!(!red.new_expression.children().iter().any(|c| c.children().iter().any(|c| c.is_clean())));
                results.push(RuleResult {
                    rule_data: rule_data.clone(),
                    reduction: red,
                });
            }
            Err(_) => {
                log::trace!(
                    "Rule attempted but not applied: {}, to expression: {} ({:?})",
                    rule_data.rule,
                    expression,
                    rule_data
                );
                stats.rewriter_rule_application_attempts =
                    Some(stats.rewriter_rule_application_attempts.unwrap() + 1);
                continue;
            }
        }
    }
    results
}

/// Chooses the first applicable rule result from a list of rule applications.
///
/// Currently, applies the rule with the highest priority.
/// If multiple rules have the same priority, logs an error message and panics.
///
/// # Parameters
/// - `results`: A slice of [`RuleResult`]s to consider.
/// -  `initial_expression`: [`Expression`] before the rule application.
///
/// # Returns
/// - `Some(<Reduction>)`: If there is at least one successful rule application, returns a [`Reduction`] to apply.
/// - `None`: If there are no successful rule applications (i.e. `results` is empty).
///
/// # Example
///
/// let rule_results = vec![rule1_result, rule2_result];
/// if let Some(reduction) = choose_rewrite(&rule_results) {
///   // Process the chosen reduction
/// }
///
fn choose_rewrite<'a>(
    results: &[RuleResult<'a>],
    initial_expression: &Expression,
) -> Option<RuleResult<'a>> {
    //in the case where multiple rules are applicable

    if !results.is_empty() {
        let mut rewrite_options: Vec<RuleResult> = Vec::new();
        for (priority, group) in &results.iter().chunk_by(|result| result.rule_data.priority) {
            let options: Vec<&RuleResult> = group.collect();
            if options.len() > 1 {
                // Multiple rules with the same priority
                let mut message = format!(
                    "Found multiple rules of the same priority {} applicable to expression: {}\n",
                    priority, initial_expression
                );
                for option in options {
                    message.push_str(&format!(
                        "- Rule: {} (from {})\n",
                        option.rule_data.rule.name, option.rule_data.rule_set.name
                    ));
                }
                bug!("{}", message);
            } else {
                // Only one rule with this priority, add it to the list
                rewrite_options.push(options[0].clone());
            }
        }

        if rewrite_options.len() > 1 {
            // Keep old behaviour: log a message and apply the highest priority rule
            let mut message = format!(
                "Found multiple rules of different priorities applicable to expression: {}\n",
                initial_expression
            );
            for option in &rewrite_options {
                message.push_str(&format!(
                    "- Rule: {} (priority {}, from {})\n",
                    option.rule_data.rule.name,
                    option.rule_data.priority,
                    option.rule_data.rule_set.name
                ));
            }
            log::warn!("{}", message);
        }
        return Some(rewrite_options[0].clone());
    }

    None
}

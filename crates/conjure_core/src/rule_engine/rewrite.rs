use std::collections::HashMap;
use std::env;

use crate::ast::ReturnType;
use crate::bug;
use crate::stats::RewriterStats;
use uniplate::Uniplate;

use crate::rule_engine::{Reduction, Rule, RuleSet};
use crate::{
    ast::Expression,
    rule_engine::resolve_rules::{get_rule_priorities, get_rules_vec},
    Model,
};

use super::rewriter_common::{log_rule_application, RewriteError, RuleResult};

/// Checks if the OPTIMIZATIONS environment variable is set to "1".
///
/// # Returns
/// - true if the environment variable is set to "1".
/// - false if the environment variable is not set or set to any other value.
fn optimizations_enabled() -> bool {
    match env::var("OPTIMIZATIONS") {
        Ok(val) => val == "1",
        Err(_) => false, // Assume optimizations are disabled if the environment variable is not set
    }
}

/// Rewrites the given model by applying a set of rules to all its constraints.
///
/// This function iteratively applies transformations to the model's constraints using the specified rule sets.
/// It returns a modified version of the model with all applicable rules applied, ensuring that any side-effects
/// such as updates to the symbol table and top-level constraints are properly reflected in the returned model.
///
/// # Parameters
/// - `model`: A reference to the [`Model`] to be rewritten. The function will clone this model to produce a modified version.
/// - `rule_sets`: A vector of references to [`RuleSet`]s that define the rules to be applied to the model's constraints.
///   Each `RuleSet` is expected to contain a collection of rules that can transform one or more constraints
///   within the model. The lifetime parameter `'a` ensures that the rules' references are valid for the
///   duration of the function execution.
///
/// # Returns
/// - `Ok(Model)`: If successful, it returns a modified copy of the [`Model`] after all applicable rules have been
///   applied. This new model includes any side-effects such as updates to the symbol table or modifications
///   to the constraints.
/// - `Err(RewriteError)`: If an error occurs during rule application (e.g., invalid rules or failed constraints),
///   it returns a [`RewriteError`] with details about the failure.
///
/// # Side-Effects
/// - When the model is rewritten, related data structures such as the symbol table (which tracks variable names and types)
///   or other top-level constraints may also be updated to reflect these changes. These updates are applied to the returned model,
///   ensuring that all related components stay consistent and aligned with the changes made during the rewrite.
/// - The function collects statistics about the rewriting process, including the number of rule applications
///   and the total runtime of the rewriter. These statistics are then stored in the model's context for
///   performance monitoring and analysis.
///
/// # Example
/// - Using `rewrite_model` with the Expression `a + min(x, y)`
///
///   Initial expression: a + min(x, y)
///   A model containing the expression is created. The variables of the model are represented by a SymbolTable and contain a,x,y.
///   The contraints of the initail model is the expression itself.
///
///   After getting the rules by their priorities and getting additional statistics the while loop of single interations is executed.
///   Details for this process can be found in [`rewrite_iteration`] documentation.
///
///   The loop is exited only when no more rules can be applied, when rewrite_iteration returns None and [`while let Some(step) = None`] occurs
///
///
///   Will result in side effects ((d<=x ^ d<=y) being the [`new_top`] and the model will now be a conjuction of that and (a+d)
///   Rewritten expression: ((a + d) ^ (d<=x ^ d<=y))
///
/// # Performance Considerations
/// - The function checks if optimizations are enabled before applying rules, which may affect the performance
///   of the rewriting process.
/// - Depending on the size of the model and the number of rules, the rewriting process might take a significant
///   amount of time. Use the statistics collected (`rewriter_run_time` and `rewriter_rule_application_attempts`)
///   to monitor and optimize performance.
///
/// # Panics
/// - This function may panic if the model's context is unavailable or if there is an issue with locking the context.
///
/// # See Also
/// - [`get_rule_priorities`]: Retrieves the priorities for the given rules.
/// - [`rewrite_iteration`]: Executes a single iteration of rewriting the model using the specified rules.
pub fn rewrite_model<'a>(
    model: &Model,
    rule_sets: &Vec<&'a RuleSet<'a>>,
) -> Result<Model, RewriteError> {
    let rule_priorities = get_rule_priorities(rule_sets)?;
    let rules = get_rules_vec(&rule_priorities);
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
    while i < new_model.constraints.len() {
        while let Some(step) = rewrite_iteration(
            &new_model.constraints[i],
            &new_model,
            &rules,
            apply_optimizations,
            &mut stats,
        ) {
            debug_assert!(is_vec_bool(&step.new_top)); // All new_top expressions should be boolean
            new_model.constraints[i] = step.new_expression.clone();
            step.apply(&mut new_model); // Apply side-effects (e.g., symbol table updates)
        }

        // If new constraints are added, continue processing them in the next iterations.
        i += 1;
    }

    stats.rewriter_run_time = Some(start.elapsed());
    model.context.write().unwrap().stats.add_rewriter_run(stats);
    Ok(new_model)
}

/// Checks if all expressions in `Vec<Expr>` are booleans. This needs to be true so
/// the vector could be conjuncted to the model.
/// # Returns
///
/// - true: all expressions are booleans (so can be conjuncted).
///
/// - false: not all expressions are booleans.
fn is_vec_bool(exprs: &[Expression]) -> bool {
    exprs
        .iter()
        .all(|expr| expr.return_type() == Some(ReturnType::Bool))
}

/// Attempts to apply a set of rules to the given expression and its sub-expressions in the model.
///
/// This function recursively traverses the provided expression, applying any applicable rules from the given set.
/// If a rule is successfully applied to the expression or any of its sub-expressions, it returns a `Reduction`
/// containing the new expression, modified top-level constraints, and any changes to symbols. If no rules can be
/// applied at any level, it returns `None`.
///
/// # Parameters
/// - `expression`: A reference to the [`Expression`] to be rewritten. This is the main expression that the function
///   attempts to modify using the given rules.
/// - `model`: A reference to the [`Model`] that provides context and additional constraints for evaluating the rules.
/// - `rules`: A vector of references to [`Rule`]s that define the transformations to apply to the expression.
/// - `apply_optimizations`: A boolean flag that indicates whether optimization checks should be applied during the rewriting process.
///   If `true`, the function skips already "clean" (fully optimized or processed) expressions and marks them accordingly
///   to avoid redundant work.
/// - `stats`: A mutable reference to [`RewriterStats`] to collect statistics about the rule application process, such as
///   the number of rules applied and the time taken for each iteration.
///
/// # Returns
/// - `Some(<Reduction>)`: A [`Reduction`] containing the new expression and any associated modifications if a rule was applied
///   to `expr` or one of its sub-expressions.
/// - `None`: If no rule is applicable to the expression or any of its sub-expressions.
///
/// # Side-Effects
/// - If `apply_optimizations` is enabled, the function will skip "clean" expressions and mark successfully rewritten
///   expressions as "dirty". This is done to avoid unnecessary recomputation of expressions that have already been
///   optimized or processed.
///
/// # Example
/// - Recursively applying [`rewrite_iteration`]  to [`a + min(x, y)`]
///
///   Initially [`if apply_optimizations && expression.is_clean()`] is not true yet since intially our expression is dirty.
///
///   [`apply_results`] returns a null vector since no rules can be applied at the top level.
///   After calling function [`children`] on the expression a vector of sub-expression [`[a, min(x, y)]`] is returned.
///
///   The function iterates through the vector of the children from the top expression and calls itself.
///
///   [rewrite_iteration] on on the child [`a`] returns None, but on [`min(x, y)`] returns a [`Reduction`] object [`red`].
///   In this case, a rule (min simplification) can apply:
///   - d is added to the SymbolTable and the variables field is updated in the model. new_top is the side effects: (d<=x ^ d<=y)
///   - [`red = Reduction::new(new_expression = d, new_top, symbols)`];
///   - [`sub[1] = red.new_expression`] - Updates the second element in the vector of sub-expressions from [`min(x, y)`] to [`d`]
///
///   Since a child expression [`min(x, y)`] was rewritten to d, the parent expression [`a + min(x, y)`] is updated with the new child [`a+d`].
///   New [`Reduction`] is returned containing the modifications
///
///   The condition [`Some(step) = Some(new reduction)`] in the while loop in [`rewrite_model`] is met -> side effects are applied.
///
///   No more rules in our example can apply to the modified model -> mark all the children as clean and return a pure [`Reduction`].
///   [`return Some(Reduction::pure(expression))`]
///
///   On the last execution of rewrite_iteration condition [`apply_optimizations && expression.is_clean()`] is met, [`None`] is returned.
///
///
/// # Notes
/// - This function works recursively, meaning it traverses all sub-expressions within the given `expression` to find the
///   first rule that can be applied. If a rule is applied, it immediately returns the modified expression and stops
///   further traversal for that branch.
fn rewrite_iteration<'a>(
    expression: &'a Expression,
    model: &'a Model,
    rules: &'a Vec<&'a Rule<'a>>,
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
        log_rule_application(&result, &expression);
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

/// Applies all the given rules to a specific expression within the model.
///
/// This function iterates through the provided rules and attempts to apply each rule to the given `expression`.
/// If a rule is successfully applied, it creates a [`RuleResult`] containing the original rule and the resulting
/// [`Reduction`]. The statistics (`stats`) are updated to reflect the number of rule application attempts and successful
/// applications.
///
/// The function does not modify the provided `expression` directly. Instead, it collects all applicable rule results
/// into a vector, which can then be used for further processing or selection (e.g., with [`choose_rewrite`]).
///
/// # Parameters
/// - `expression`: A reference to the [`Expression`] that will be evaluated against the given rules. This is the main
///   target for rule transformations and is expected to remain unchanged during the function execution.
/// - `model`: A reference to the [`Model`] that provides context for rule evaluation, such as constraints and symbols.
///   Rules may depend on information in the model to determine if they can be applied.
/// - `rules`: A vector of references to [`Rule`]s that define the transformations to be applied to the expression.
///   Each rule is applied independently, and all applicable rules are collected.
/// - `stats`: A mutable reference to [`RewriterStats`] used to track statistics about rule application, such as
///   the number of attempts and successful applications.
///
/// # Returns
/// - A `Vec<RuleResult>` containing all rule applications that were successful. Each element in the vector represents
///   a rule that was applied to the given `expression` along with the resulting transformation.
/// - An empty vector if no rules were applicable to the expression.
///
/// # Side-Effects
/// - The function updates the provided `stats` with the number of rule application attempts and successful applications.
/// - Debug or trace logging may be performed to track which rules were applicable or not for a given expression.
///
/// # Example
///
/// let applicable_rules = apply_all_rules(&expr, &model, &rules, &mut stats);
/// if !applicable_rules.is_empty() {
///     for result in applicable_rules {
///         println!("Rule applied: {:?}", result.rule);
///     }
/// }
///
///
/// # Notes
/// - This function does not modify the input `expression` or `model` directly. The returned `RuleResult` vector
///   provides information about successful transformations, allowing the caller to decide how to process them.
/// - The function performs independent rule applications. If rules have dependencies or should be applied in a
///   specific order, consider handling that logic outside of this function.
///
/// # See Also
/// - [`choose_rewrite`]: Chooses a single reduction from the rule results provided by `apply_all_rules`.
fn apply_all_rules<'a>(
    expression: &'a Expression,
    model: &'a Model,
    rules: &'a Vec<&'a Rule<'a>>,
    stats: &mut RewriterStats,
) -> Vec<RuleResult<'a>> {
    let mut results = Vec::new();
    for rule in rules {
        match rule.apply(expression, model) {
            Ok(red) => {
                stats.rewriter_rule_application_attempts =
                    Some(stats.rewriter_rule_application_attempts.unwrap() + 1);
                stats.rewriter_rule_applications =
                    Some(stats.rewriter_rule_applications.unwrap() + 1);
                // Assert no clean children
                // assert!(!red.new_expression.children().iter().any(|c| c.is_clean()), "Rule that caused assertion to fail: {:?}", rule.name);
                // assert!(!red.new_expression.children().iter().any(|c| c.children().iter().any(|c| c.is_clean())));
                results.push(RuleResult {
                    rule,
                    reduction: red,
                });
            }
            Err(_) => {
                log::trace!(
                    "Rule attempted but not applied: {} ({:?}), to expression: {}",
                    rule.name,
                    rule.rule_sets,
                    expression
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
/// This function selects a reduction from the provided `RuleResult` list, prioritizing the first rule
/// that successfully transforms the expression. This strategy can be modified in the future to incorporate
/// more complex selection criteria, such as prioritizing rules based on cost, complexity, or other heuristic metrics.
///
/// The function also checks the priorities of all the applicable rules and detects if there are multiple rules of the same proirity
///
/// # Parameters
/// - `results`: A slice of [`RuleResult`] containing potential rule applications to be considered. Each element
///   represents a rule that was successfully applied to the expression, along with the resulting transformation.
/// -  `initial_expression`: [`Expression`] before the rule tranformation.
///
/// # Returns
/// - `Some(<Reduction>)`: Returns a [`Reduction`] representing the first rule's application if there is at least one
///   rule that produced a successful transformation.
/// - `None`: If no rule applications are available in the `results` slice (i.e., it is empty), it returns `None`.
///
/// # Example
///
/// let rule_results = vec![rule1_result, rule2_result];
/// if let Some(reduction) = choose_rewrite(&rule_results) {
/// Process the chosen reduction
/// }
///
fn choose_rewrite<'a>(
    results: &[RuleResult<'a>],
    initial_expression: &Expression,
) -> Option<RuleResult<'a>> {
    //in the case where multiple rules are applicable
    if results.len() > 1 {
        let expr = results[0].reduction.new_expression.clone();
        let rules: Vec<_> = results.iter().map(|result| &result.rule).collect();

        check_priority(rules.clone(), initial_expression, &expr);
    }

    if results.is_empty() {
        return None;
    }

    // Return the first result for now
    Some(results[0].clone())
}

/// Function filters all the applicable rules based on their priority.
/// In the case where there are multiple rules of the same prioriy, a bug! is thrown listing all those duplicates.
/// Otherwise, if there are multiple rules applicable but they all have different priorities, a warning message is dispalyed.
///
/// # Parameters
/// - `rules`: a vector of [`Rule`] containing all the applicable rules and their metadata for a specific expression.
/// - `initial_expression`: [`Expression`] before rule the tranformation.
/// - `new_expr`: [`Expression`] after the rule transformation.
///
fn check_priority<'a>(
    rules: Vec<&&Rule<'_>>,
    initial_expr: &'a Expression,
    new_expr: &'a Expression,
) {
    //getting the rule sets from the applicable rules
    let rule_sets: Vec<_> = rules.iter().map(|rule| &rule.rule_sets).collect();

    //a map with keys being rule priorities and their values neing all the rules of that priority found in the rule_sets
    let mut rules_by_priorities: HashMap<u16, Vec<&str>> = HashMap::new();

    //iterates over each rule_set and groups by the rule priority
    for rule_set in &rule_sets {
        if let Some((name, priority)) = rule_set.first() {
            rules_by_priorities
                .entry(*priority)
                .or_default()
                .push(*name);
        }
    }

    //filters the map, retaining only entries where there is more than 1 rule of the same priority
    let duplicate_rules: HashMap<u16, Vec<&str>> = rules_by_priorities
        .into_iter()
        .filter(|(_, group)| group.len() > 1)
        .collect();

    if !duplicate_rules.is_empty() {
        //accumulates all duplicates into a formatted message
        let mut message = format!("Found multiple rules of the same priority applicable to to expression: {} \n resulting in expression: {}", initial_expr, new_expr);
        for (priority, rules) in &duplicate_rules {
            message.push_str(&format!("Priority {:?} \n Rules: {:?}", priority, rules));
        }
        bug!("{}", message);

    //no duplicate rules of the same priorities were found in the set of applicable rules
    } else {
        log::warn!("Multiple rules of different priorities are applicable to expression {} \n resulting in expression: {}
        \n Rules{:?}", initial_expr, new_expr, rules)
    }
}

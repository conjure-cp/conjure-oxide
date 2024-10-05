use std::env;
use std::fmt::Display;

use thiserror::Error;

use crate::stats::RewriterStats;
use uniplate::Uniplate;

use crate::rule_engine::{Reduction, Rule, RuleSet};
use crate::{
    ast::Expression,
    rule_engine::resolve_rules::{
        get_rule_priorities, get_rules_vec, ResolveRulesError as ResolveError,
    },
    Model,
};

#[derive(Debug)]
struct RuleResult<'a> {
    rule: &'a Rule<'a>,
    reduction: Reduction,
}

/// Represents errors that can occur during the model rewriting process.
///
/// This enum captures errors that occur when trying to resolve or apply rules in the model.
#[derive(Debug, Error)]
pub enum RewriteError {
    ResolveRulesError(ResolveError),
}

impl Display for RewriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RewriteError::ResolveRulesError(e) => write!(f, "Error resolving rules: {}", e),
        }
    }
}

impl From<ResolveError> for RewriteError {
    fn from(error: ResolveError) -> Self {
        RewriteError::ResolveRulesError(error)
    }
}

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
/// ```rust
///     // Initial expression: a + min(x, y)
///     let initial_constraints = Expression::new(Sum(Metadata, Vec<Expression(a + min(x, y)))),
///     let variables = SymbolTable::new  //a hashamp that will hold variables a,x,y
///    
///     // Create a model containing the expression
///     let model = Model::new(variables, initial_constraints, some_context, next_var);
///
///     // Define rule sets with simplification rules for the benefit of the example
///     let rule_set_1 = RuleSet::new(vec![
///         Rule::new("min(x, y) => d if d <= x and d<=y "),
///         Rule::new("a + 0 => a"),
///     ]);
///
///     let rule_set_2 = RuleSet::new(vec![
///         Rule::new("x * 1 => x"),
///     ]);
///
///     // Apply the rewriting model
///     let optimized_model = rewrite_model(&model, &vec![&rule_set_1, &rule_set_2])?;
///
/// //Inside rewrite_model
///     //After getting the rules by their priorities and getting additional statistics the while loop of single interations
///     //is executed
///     while let Some(step) = None // the loop is exited only when no more rules can be applied, when rewrite_iteration returns None
///     
///     //will result is side effects ((d<=x ^ d<=y) being the new_top and the model will now be a conjuction of that and (a+d)
///     step.(&mut new_model)
///      
///     //Rewritten expression: ((a + d) ^ (d<=x ^ d<=y))/
/// ```
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
    while let Some(step) = rewrite_iteration(
        &new_model.constraints,
        &new_model,
        &rules,
        apply_optimizations,
        &mut stats,
    ) {
        step.apply(&mut new_model); // Apply side-effects (e.g. symbol table updates)
    }
    stats.rewriter_run_time = Some(start.elapsed());
    model.context.write().unwrap().stats.add_rewriter_run(stats);
    Ok(new_model)
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
/// # Example of rewtire iteration for the expression `a + min(x, y)`
/// ```rust
///  //Initially
///  if apply_optimizations && expression.is_clean() //is not true yet since intially our expression is dirty
///
///  rule_results = null //apply_results returns a null vector since no rules can be applied at the top level
///  let mut sub = expression.children();  // sub = [a, min(x, y)] - vector of subexpressions
///
/// //the function iterates through the vector of the children of the top expression and calls itself
///
/// //rewrite_iteration on a returns None, but on min(x, y) returns a Reduction object red. In this case, Rule 1 (min simplification) might apply:
/// //d is added to the SymbolTable and the variables field is updated in the model. new_top is the side effects: (d<=x ^ d<=y)
///  let red = Reduction::new(new_expression = d, new_top, symbols);
///  sub[1] = red.new_expression;  // Update `min(x, y)` to `d`
///
/// //Since a child expression (min(x, y)) was rewritten to x, the parent expression (a + min(x, y)) is updated with the new child:
///  let res = expression.with_children(sub.clone());  // res = `a + d`
///  return Some(Reduction::new(res, red.new_top, red.symbols));  // `a + d`,
///
///  //the condition in the while loop in rewrite_model is met -> side effects are applied
///
///  //no more rules in our example can apply to the modified model -> mark all the children as clean and return a pure reduction
///  return Some(Reduction::pure(expression));
///    
/// //on the last execution of rewrite_iteration
///  if apply_optimizations && expression.is_clean() {
///      return None; //the while loop is rewrite_model is exited
/// }
///
/// ```
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
    if let Some(new) = choose_rewrite(&rule_results) {
        // If a rule is applied, mark the expression as dirty
        return Some(new);
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
/// ```rust
/// let applicable_rules = apply_all_rules(&expr, &model, &rules, &mut stats);
/// if !applicable_rules.is_empty() {
///     for result in applicable_rules {
///         println!("Rule applied: {:?}", result.rule);
///     }
/// }
/// ```
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
                log::trace!(target: "file", "Rule applicable: {:?}, to Expression: {:?}, resulting in: {:?}", rule, expression, red.new_expression);
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
                log::trace!(target: "file", "Rule attempted but not applied: {:?}, to Expression: {:?}", rule, expression);
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
/// # Parameters
/// - `results`: A slice of [`RuleResult`] containing potential rule applications to be considered. Each element
///   represents a rule that was successfully applied to the expression, along with the resulting transformation.
///
/// # Returns
/// - `Some(<Reduction>)`: Returns a [`Reduction`] representing the first rule's application if there is at least one
///   rule that produced a successful transformation.
/// - `None`: If no rule applications are available in the `results` slice (i.e., it is empty), it returns `None`.
///
/// # Example
/// ```rust
/// let rule_results = vec![rule1_result, rule2_result];
/// if let Some(reduction) = choose_rewrite(&rule_results) {
///     // Process the chosen reduction
/// }
/// ```
fn choose_rewrite(results: &[RuleResult]) -> Option<Reduction> {
    if results.is_empty() {
        return None;
    }
    // Return the first result for now
    Some(results[0].reduction.clone())
}

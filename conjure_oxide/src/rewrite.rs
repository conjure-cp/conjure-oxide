use conjure_core::ast::{Expression, Model};
use conjure_core::rule::{Reduction, Rule};
use conjure_rules::get_rules;

struct RuleResult<'a> {
    rule: Rule<'a>,
    reduction: Reduction,
}

/// # Returns
/// - A new expression after applying the rules to `expression` and its sub-expressions.
/// - The same expression if no rules are applicable.
pub fn rewrite(expression: &Expression, model: &Model) -> Expression {
    let rules = get_rules();
    let mut new = expression.clone();
    while let Some(step) = rewrite_iteration(&new, model, &rules) {
        new = step;
    }
    new
}

/// # Returns
/// - Some(<new_expression>) after applying the first applicable rule to `expr` or a sub-expression.
/// - None if no rule is applicable to the expression or any sub-expression.
fn rewrite_iteration<'a>(
    expression: &'a Expression,
    model: &'a Model,
    rules: &'a Vec<Rule<'a>>,
) -> Option<Expression> {
    let rule_results = apply_all_rules(expression, model, rules);
    if let Some(new) = choose_rewrite(&rule_results) {
        return Some(new);
    } else {
        match expression.sub_expressions() {
            None => {}
            Some(mut sub) => {
                for i in 0..sub.len() {
                    if let Some(new) = rewrite_iteration(sub[i], model, rules) {
                        sub[i] = &new;
                        return Some(expression.with_sub_expressions(sub));
                    }
                }
            }
        }
    }
    None // No rules applicable to this branch of the expression
}

/// # Returns
/// - A list of RuleResults after applying all rules to `expression`.
/// - An empty list if no rules are applicable.
fn apply_all_rules<'a>(
    expression: &'a Expression,
    model: &'a Model,
    rules: &'a Vec<Rule<'a>>,
) -> Vec<RuleResult<'a>> {
    let mut results = Vec::new();
    for rule in rules {
        match rule.apply(expression, model) {
            Ok(red) => {
                results.push(RuleResult {
                    rule: rule.clone(),
                    reduction: red,
                });
            }
            Err(_) => continue,
        }
    }
    results
}

/// # Returns
/// - Some(<new_expression>) after applying the first rule in `results`.
/// - None if `results` is empty.
fn choose_rewrite(results: &Vec<RuleResult>) -> Option<Expression> {
    if results.is_empty() {
        return None;
    }
    // Return the first result for now
    // println!("Applying rule: {:?}", results[0].rule);
    Some(results[0].reduction.new_expression.clone())
}

/// This rewrites the model by applying the rules to all constraints.
/// # Returns
/// - A new model with rewritten constraints.
/// - The same model if no rules are applicable.
pub fn rewrite_model(model: &Model) -> Model {
    let mut new_model = model.clone();

    new_model.constraints = rewrite(&model.constraints, model);

    new_model
}

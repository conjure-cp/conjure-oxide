use conjure_core::ast::{Expression, Model};
use conjure_core::rule::{Reduction, Rule};
use conjure_rules::get_rules;

struct RuleResult<'a> {
    rule: Rule<'a>,
    reduction: Reduction,
}

/// Rewrites the model by applying the rules to all constraints.
///
/// Any side-effects such as symbol table updates and top-level constraints are applied to the returned model.
///
/// # Returns
/// A copy of the model after all, if any, possible rules are applied to its constraints.
pub fn rewrite_model(model: &Model) -> Model {
    let rules = get_rules();
    let mut new_model = model.clone();
    while let Some(step) = rewrite_iteration(&new_model.constraints, model, &rules) {
        new_model.variables.extend(step.symbols); // Add new assignments to the symbol table
        if step.new_top.is_nothing() {
            new_model.constraints = step.new_expression.clone();
        } else {
            new_model.constraints = match step.new_expression {
                // Avoid creating a nested conjunction
                Expression::And(mut and) => {
                    and.push(step.new_top.clone());
                    Expression::And(and)
                }
                _ => Expression::And(vec![step.new_expression.clone(), step.new_top]),
            };
        }
    }
    new_model
}

/// # Returns
/// - Some(<new_expression>) after applying the first applicable rule to `expr` or a sub-expression.
/// - None if no rule is applicable to the expression or any sub-expression.
fn rewrite_iteration<'a>(
    expression: &'a Expression,
    model: &'a Model,
    rules: &'a Vec<Rule<'a>>,
) -> Option<Reduction> {
    let rule_results = apply_all_rules(expression, model, rules);
    if let Some(new) = choose_rewrite(&rule_results) {
        return Some(new);
    } else {
        match expression.sub_expressions() {
            None => {}
            Some(mut sub) => {
                for i in 0..sub.len() {
                    if let Some(red) = rewrite_iteration(sub[i], model, rules) {
                        sub[i] = &red.new_expression;
                        return Some(Reduction::new(
                            expression.clone().with_sub_expressions(sub),
                            red.new_top,
                            red.symbols,
                        ));
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
/// - Some(<reduction>) after applying the first rule in `results`.
/// - None if `results` is empty.
fn choose_rewrite(results: &Vec<RuleResult>) -> Option<Reduction> {
    if results.is_empty() {
        return None;
    }
    // Return the first result for now
    // println!("Applying rule: {:?}", results[0].rule);
    Some(results[0].reduction.clone())
}

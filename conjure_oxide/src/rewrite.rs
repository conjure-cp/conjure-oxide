use conjure_core::ast::Expression;
use conjure_core::rule::Rule;
use conjure_rules::get_rules;

struct RuleResult<'a> {
    rule: Rule<'a>,
    new_expression: Expression,
}

pub fn rewrite(expression: &Expression) -> Expression {
    let rules = get_rules();
    let mut new = expression.clone(); 
    while let Some(step) = rewrite_iteration(&new, &rules) {
        new = step;
    }
    new
}

fn apply_all_rules<'a>(expression: &'a Expression, rules: &'a Vec<Rule<'a>>) -> Vec<RuleResult<'a>> {
    let mut results = Vec::new();
    for rule in rules {
        match rule.apply(&expression) {
            Ok(new) => {
                results.push(RuleResult {
                    rule: rule.clone(),
                    new_expression: new,
                });
            }
            Err(_) => continue,
        }
    }
    results
}

fn choose_rewrite<'a>(results: &Vec<RuleResult<'a>>) -> Option<Expression> {
    if results.len() == 0 {
        return None;
    }
    // Return the first result for now
    Some(results[0].new_expression.clone())
}

/// # Returns
/// - Some(<new_expression>) after applying the first applicable rule to `expr` or a sub-expression.
/// - None if no rule is applicable to the expression or any sub-expression.
fn rewrite_iteration<'a>(expression: &'a Expression, rules: &'a Vec<Rule<'a>>) -> Option<Expression> {
    let rule_results = apply_all_rules(expression, rules);
    if let Some(new) = choose_rewrite(&rule_results) {
        return Some(new);
    } else {
        let mut sub = expression.sub_expressions();
        for i in 0..sub.len() {
            if let Some(new) = rewrite_iteration(sub[i], rules) {
                sub[i] = &new;
                return Some(expression.with_sub_expressions(sub));
            }
        }
    }
    None // No rules applicable to this branch of the expression
}

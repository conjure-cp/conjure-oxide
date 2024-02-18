use conjure_core::{ast::Expression as Expr, rule::RuleApplicationError};
use conjure_rules::{register_rule, register_rule_set};

/***********************************************************************************/
/*        This file contains rules for converting logic expressions to CNF         */
/***********************************************************************************/

register_rule_set!("CNF", 20, ("Base"));

/**
* Distribute `not` over `and` (De Morgan's Law):

* ```text
* not(and(a, b)) = or(not a, not b)
* ```
 */
#[register_rule(("CNF", 20))] // ToDo: not sure about the priority - discuss
fn distribute_not_over_and(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Not(contents) => match contents.as_ref() {
            Expr::And(exprs) => {
                let mut new_exprs = Vec::new();
                for e in exprs {
                    new_exprs.push(Expr::Not(Box::new(e.clone())));
                }
                Ok(Expr::Or(new_exprs))
            }
            _ => Err(RuleApplicationError::RuleNotApplicable),
        },
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
* Distribute `not` over `or` (De Morgan's Law):

* ```text
* not(or(a, b)) = and(not a, not b)
* ```
 */
#[register_rule(("CNF", 20))]
fn distribute_not_over_or(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Not(contents) => match contents.as_ref() {
            Expr::Or(exprs) => {
                let mut new_exprs = Vec::new();
                for e in exprs {
                    new_exprs.push(Expr::Not(Box::new(e.clone())));
                }
                Ok(Expr::And(new_exprs))
            }
            _ => Err(RuleApplicationError::RuleNotApplicable),
        },
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
* Apply the Distributive Law to expressions like `Or([..., And(a, b)])`

* ```text
* or(and(a, b), c) = and(or(a, c), or(b, c))
* ```
 */
#[register_rule(("CNF", 20))]
fn distribute_or_over_and(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    fn find_and(exprs: &Vec<Expr>) -> Option<usize> {
        // ToDo: may be better to move this to some kind of utils module?
        for (i, e) in exprs.iter().enumerate() {
            if let Expr::And(_) = e {
                return Some(i);
            }
        }
        None
    }

    match expr {
        Expr::Or(exprs) => match find_and(exprs) {
            Some(idx) => {
                let mut rest = exprs.clone();
                let and_expr = rest.remove(idx);

                match and_expr {
                    Expr::And(and_exprs) => {
                        let mut new_and_contents = Vec::new();

                        for e in and_exprs {
                            // ToDo: Cloning everything may be a bit inefficient - discuss
                            let mut new_or_contents = rest.clone();
                            new_or_contents.push(e.clone());
                            new_and_contents.push(Expr::Or(new_or_contents))
                        }

                        Ok(Expr::And(new_and_contents))
                    }
                    _ => Err(RuleApplicationError::RuleNotApplicable),
                }
            }
            None => Err(RuleApplicationError::RuleNotApplicable),
        },
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

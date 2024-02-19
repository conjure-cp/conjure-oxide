use conjure_core::{
    ast::Expression as Expr,
    rule::{ApplicationError, ApplicationResult, Reduction},
};
use conjure_rules::register_rule;

/***********************************************************************************/
/*        This file contains rules for converting logic expressions to CNF         */
/***********************************************************************************/

/**
* Distribute `not` over `and` (De Morgan's Law):

* ```text
* not(and(a, b)) = or(not a, not b)
* ```
 */
#[register_rule]
fn distribute_not_over_and(expr: &Expr) -> ApplicationResult {
    match expr {
        Expr::Not(contents) => match contents.as_ref() {
            Expr::And(exprs) => {
                let mut new_exprs = Vec::new();
                for e in exprs {
                    new_exprs.push(Expr::Not(Box::new(e.clone())));
                }
                Ok(Reduction::pure(Expr::Or(new_exprs)))
            }
            _ => Err(ApplicationError::RuleNotApplicable),
        },
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
* Distribute `not` over `or` (De Morgan's Law):

* ```text
* not(or(a, b)) = and(not a, not b)
* ```
 */
#[register_rule]
fn distribute_not_over_or(expr: &Expr) -> ApplicationResult {
    match expr {
        Expr::Not(contents) => match contents.as_ref() {
            Expr::Or(exprs) => {
                let mut new_exprs = Vec::new();
                for e in exprs {
                    new_exprs.push(Expr::Not(Box::new(e.clone())));
                }
                Ok(Reduction::pure(Expr::And(new_exprs)))
            }
            _ => Err(ApplicationError::RuleNotApplicable),
        },
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
* Apply the Distributive Law to expressions like `Or([..., And(a, b)])`

* ```text
* or(and(a, b), c) = and(or(a, c), or(b, c))
* ```
 */
#[register_rule]
fn distribute_or_over_and(expr: &Expr) -> ApplicationResult {
    fn find_and(exprs: &Vec<Expr>) -> Option<usize> {
        // ToDo: may be better to move this to some kind of utils module?
        for (i, e) in exprs.iter().enumerate() {
            match e {
                Expr::And(_) => return Some(i),
                _ => (),
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

                        Ok(Reduction::pure(Expr::And(new_and_contents)))
                    }
                    _ => Err(ApplicationError::RuleNotApplicable),
                }
            }
            None => Err(ApplicationError::RuleNotApplicable),
        },
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

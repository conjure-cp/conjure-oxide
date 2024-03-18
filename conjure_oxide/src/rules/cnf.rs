use crate::{
    ast::Expression as Expr, register_rule, register_rule_set, ApplicationError, ApplicationResult,
    Model, Reduction, SolverFamily,
};

/***********************************************************************************/
/*        This file contains rules for converting logic expressions to CNF         */
/***********************************************************************************/

register_rule_set!("CNF", 100, ("Base"), (SolverFamily::SAT));

/**
* Distribute `not` over `and` (De Morgan's Law):

* ```text
* not(and(a, b)) = or(not a, not b)
* ```
 */
#[register_rule(("CNF", 100))]
fn distribute_not_over_and(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Not(_, contents) => match contents.as_ref() {
            Expr::And(metadata, exprs) => {
                let mut new_exprs = Vec::new();
                for e in exprs {
                    new_exprs.push(Expr::Not(metadata.clone(), Box::new(e.clone())));
                }
                Ok(Reduction::pure(Expr::Or(metadata.clone(), new_exprs)))
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
#[register_rule(("CNF", 100))]
fn distribute_not_over_or(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Not(_, contents) => match contents.as_ref() {
            Expr::Or(metadata, exprs) => {
                let mut new_exprs = Vec::new();
                for e in exprs {
                    new_exprs.push(Expr::Not(metadata.clone(), Box::new(e.clone())));
                }
                Ok(Reduction::pure(Expr::And(metadata.clone(), new_exprs)))
            }
            _ => Err(ApplicationError::RuleNotApplicable),
        },
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

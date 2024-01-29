use conjure_core::{ast::Expression as Expr, rule::RuleApplicationError};
use conjure_rules::register_rule;

// #[register_rule]
// fn identity(expr: &Expr) -> Result<Expr, RuleApplicationError> {
//     Ok(expr.clone())
// }

#[register_rule]
fn sum_constants(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Sum(exprs) => {
            let mut sum = 0;
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Expr::ConstantInt(i) => {
                        sum += i;
                        changed = true;
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(RuleApplicationError::RuleNotApplicable);
            }
            new_exprs.push(Expr::ConstantInt(sum));
            Ok(Expr::Sum(new_exprs)) // Let other rules handle only one Expr being contained in the sum
        }
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

#[register_rule]
fn unwrap_sum(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Sum(exprs) if (exprs.len() == 1) => Ok(exprs[0].clone()),
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

#[register_rule]
fn flatten_sum_geq(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Geq(a, b) => {
            let exprs = match a.as_ref() {
                Expr::Sum(exprs) => Ok(exprs),
                _ => Err(RuleApplicationError::RuleNotApplicable),
            }?;
            Ok(Expr::SumGeq(exprs.clone(), b.clone()))
        }
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

#[register_rule]
fn sum_leq_to_sumleq(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Leq(a, b) => {
            let exprs = match a.as_ref() {
                Expr::Sum(exprs) => Ok(exprs),
                _ => Err(RuleApplicationError::RuleNotApplicable),
            }?;
            Ok(Expr::SumLeq(exprs.clone(), b.clone()))
        }
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

#[register_rule]
fn lt_to_ineq(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Lt(a, b) => Ok(Expr::Ineq(
            a.clone(),
            b.clone(),
            Box::new(Expr::ConstantInt(-1)),
        )),
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
 * Unwrap nested `or`
 * ```text
 * or(or(a, b), c) = or(a, b, c)
 * ```
*/
#[register_rule]
fn unwrap_nested_or(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Or(exprs) => {
            let mut new_exprs = Vec::new();
            for e in exprs {
                match e.as_ref() {
                    Expr::Or(exprs) => {
                        for e in exprs {
                            new_exprs.push(e.clone());
                        }
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            Ok(Expr::Or(new_exprs))
        }
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
 * Unwrap nested `and`
 * ```text
 * and(and(a, b), c) = and(a, b, c)
 * ```
*/
#[register_rule]
fn unwrap_nested_and(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::And(exprs) => {
            let mut new_exprs = Vec::new();
            for e in exprs {
                match e.as_ref() {
                    Expr::And(exprs) => {
                        for e in exprs {
                            new_exprs.push(e.clone());
                        }
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            Ok(Expr::And(new_exprs))
        }
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
 * Distribute `not` over `or`:
 * ```text
 * not (a or b) = (not a) and (not b)
 * ```
 */
#[register_rule]
fn distribute_not_or(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Not(expression) => match expression.as_ref() {
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
 * Distribute `not` over `and`:
 * ```text

 * not (a and b) = (not a) or (not b)
 * ```
*/
#[register_rule]
fn distribute_not_and(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Not(expression) => match expression.as_ref() {
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

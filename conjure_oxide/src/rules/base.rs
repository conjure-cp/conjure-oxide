use conjure_core::{ast::Constant as Const, ast::Expression as Expr, rule::RuleApplicationError};
use conjure_rules::register_rule;

/*****************************************************************************/
/*        This file contains basic rules for simplifying expressions         */
/*****************************************************************************/

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
                    Expr::Constant(Const::Int(i)) => {
                        sum += i;
                        changed = true;
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(RuleApplicationError::RuleNotApplicable);
            }
            new_exprs.push(Expr::Constant(Const::Int(sum)));
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
            Box::new(Expr::Constant(Const::Int(-1))),
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
            let mut changed = false;
            for e in exprs {
                match e {
                    Expr::Or(exprs) => {
                        changed = true;
                        for e in exprs {
                            new_exprs.push(e.clone());
                        }
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(RuleApplicationError::RuleNotApplicable);
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
            let mut changed = false;
            for e in exprs {
                match e {
                    Expr::And(exprs) => {
                        changed = true;
                        for e in exprs {
                            new_exprs.push(e.clone());
                        }
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(RuleApplicationError::RuleNotApplicable);
            }
            Ok(Expr::And(new_exprs))
        }
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
* Remove double negation:

* ```text
* not(not(a)) = a
* ```
 */
#[register_rule]
fn remove_double_negation(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Not(contents) => match contents.as_ref() {
            Expr::Not(expr_box) => Ok(*expr_box.clone()),
            _ => Err(RuleApplicationError::RuleNotApplicable),
        },
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
 * Remove trivial `and` (only one element):
 * ```text
 * and([a]) = a
 * ```
 */
#[register_rule]
fn remove_trivial_and(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::And(exprs) => {
            if exprs.len() == 1 {
                return Ok(exprs[0].clone());
            }
            Err(RuleApplicationError::RuleNotApplicable)
        }
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
 * Remove trivial `or` (only one element):
 * ```text
 * or([a]) = a
 * ```
 */
#[register_rule]
fn remove_trivial_or(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Or(exprs) => {
            if exprs.len() == 1 {
                return Ok(exprs[0].clone());
            }
            Err(RuleApplicationError::RuleNotApplicable)
        }
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
 * Remove constant bools from or expressions
 * ```text
 * or([true, a]) = true
 * or([false, a]) = a
 * ```
 */
#[register_rule]
fn remove_constants_from_or(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Or(exprs) => {
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Expr::Constant(Const::Bool(val)) => {
                        if *val {
                            // If we find a true, the whole expression is true
                            return Ok(Expr::Constant(Const::Bool(true)));
                        } else {
                            // If we find a false, we can ignore it
                            changed = true;
                        }
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(RuleApplicationError::RuleNotApplicable);
            }
            Ok(Expr::Or(new_exprs))
        }
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
 * Remove constant bools from and expressions
 * ```text
 * and([true, a]) = a
 * and([false, a]) = false
 * ```
 */
#[register_rule]
fn remove_constants_from_and(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::And(exprs) => {
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Expr::Constant(Const::Bool(val)) => {
                        if !*val {
                            // If we find a false, the whole expression is false
                            return Ok(Expr::Constant(Const::Bool(false)));
                        } else {
                            // If we find a true, we can ignore it
                            changed = true;
                        }
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(RuleApplicationError::RuleNotApplicable);
            }
            Ok(Expr::And(new_exprs))
        }
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
 * Evaluate Not expressions with constant bools
 * ```text
 * not(true) = false
 * not(false) = true
 * ```
 */
#[register_rule]
fn evaluate_constant_not(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Not(contents) => match contents.as_ref() {
            Expr::Constant(Const::Bool(val)) => Ok(Expr::Constant(Const::Bool(!val))),
            _ => Err(RuleApplicationError::RuleNotApplicable),
        },
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

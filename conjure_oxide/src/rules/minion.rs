use crate::rules::base::flatten_nested_sum;
use conjure_core::{ast::Expression as Expr, rule::RuleApplicationError};
use conjure_rules::register_rule;

fn sum_to_vector(expr: &Expr) -> Result<Vec<Expr>, RuleApplicationError> {
    match flatten_nested_sum(expr) {
        // ToDo [HACK]: we do not have rule priority yet, so no way to ensure that nested sums get flattened before we reach here
        Ok(new_expr) => match new_expr {
            Expr::Sum(exprs) => Ok(exprs.clone()),
            _ => Err(RuleApplicationError::RuleNotApplicable),
        },
        Err(_) => match expr {
            Expr::Sum(exprs) => Ok(exprs.clone()),
            _ => Err(RuleApplicationError::RuleNotApplicable),
        },
    }
}

/**
 * Convert a Geq to a SumGeq if the left hand side is a sum:
 * ```text
 * sum([a, b, c]) >= d => sum_geq([a, b, c], d)
 * ```
 */
#[register_rule]
fn flatten_sum_geq(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Geq(a, b) => {
            let exprs = sum_to_vector(a)?;
            Ok(Expr::SumGeq(exprs, b.clone()))
        }
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
 * Convert a Leq to a SumLeq if the left hand side is a sum:
 * ```text
 * sum([a, b, c]) <= d => sum_leq([a, b, c], d)
 * ```
 */
#[register_rule]
fn sum_leq_to_sumleq(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Leq(a, b) => {
            let exprs = sum_to_vector(a)?;
            Ok(Expr::SumLeq(exprs, b.clone()))
        }
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
 * Convert a 'Eq(Sum([...]))' to a SumEq
 * ```text
 * eq(sum([a, b]), c) => sumeq([a, b], c)
 * ```
*/
#[register_rule]
fn sum_eq_to_sumeq(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Eq(a, b) => {
            let exprs = sum_to_vector(a)?;
            Ok(Expr::SumEq(exprs, b.clone()))
        }
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
 * Convert a `SumEq` to an `And(SumGeq, SumLeq)`
 * This is a workaround for Minion not having support for a flat "equals" operation on sums
 * ```text
 * sumeq([a, b], c) -> watched_and({
 *   sumleq([a, b], c),
 *   sumgeq([a, b], c)
 * })
 * ```
 * I. e.
 * ```text
 * ((a + b) >= c) && ((a + b) <= c)
 * a + b = c
 * ```
 */
#[register_rule]
fn sumeq_to_minion(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::SumEq(exprs, eq_to) => Ok(Expr::And(vec![
            Expr::SumGeq(exprs.clone(), Box::from(*eq_to.clone())),
            Expr::SumLeq(exprs.clone(), Box::from(*eq_to.clone())),
        ])),
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
* Convert a Lt to an Ineq:

* ```text
* a < b => a - b < -1
* ```
*
* Note: minion does not support strict inequalities, so we simulate it with Ineq(a, b, -1)
*/
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

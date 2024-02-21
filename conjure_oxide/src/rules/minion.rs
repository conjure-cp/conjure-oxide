use conjure_core::{
    ast::Constant as Const, ast::Expression as Expr, metadata::Metadata, rule::RuleApplicationError,
};
use conjure_rules::{register_rule, register_rule_set};

/************************************************************************/
/*        Rules for translating to Minion-supported constraints         */
/************************************************************************/

register_rule_set!("Minion", 100, ("Base"));

fn is_nested_sum(exprs: &Vec<Expr>) -> bool {
    for e in exprs {
        if let Expr::Sum(_, _) = e {
            return true;
        }
    }
    false
}

/**
 * Helper function to get the vector of expressions from a sum (or error if it's a nested sum - we need to flatten it first)
 */
fn sum_to_vector(expr: &Expr) -> Result<Vec<Expr>, RuleApplicationError> {
    match expr {
        Expr::Sum(metadata, exprs) => {
            if is_nested_sum(exprs) {
                Err(RuleApplicationError::RuleNotApplicable)
            } else {
                Ok(exprs.clone())
            }
        }
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
 * Convert a Geq to a SumGeq if the left hand side is a sum:
 * ```text
 * sum([a, b, c]) >= d => sum_geq([a, b, c], d)
 * ```
 */
#[register_rule(("Minion", 100))]
fn flatten_sum_geq(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Geq(metadata, a, b) => {
            let exprs = sum_to_vector(a)?;
            Ok(Expr::SumGeq(metadata.clone(), exprs, b.clone()))
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
#[register_rule(("Minion", 100))]
fn sum_leq_to_sumleq(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Leq(metadata, a, b) => {
            let exprs = sum_to_vector(a)?;
            Ok(Expr::SumLeq(metadata.clone(), exprs, b.clone()))
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
#[register_rule(("Minion", 100))]
fn sum_eq_to_sumeq(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Eq(metadata, a, b) => {
            let exprs = sum_to_vector(a)?;
            Ok(Expr::SumEq(metadata.clone(), exprs, b.clone()))
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
#[register_rule(("Minion", 100))]
fn sumeq_to_minion(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::SumEq(metadata, exprs, eq_to) => Ok(Expr::And(
            metadata.clone(),
            vec![
                Expr::SumGeq(metadata.clone(), exprs.clone(), Box::from(*eq_to.clone())),
                Expr::SumLeq(metadata.clone(), exprs.clone(), Box::from(*eq_to.clone())),
            ],
        )),
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
* Convert a Lt to an Ineq:

* ```text
* a < b => a - b < -1
* ```
*/
#[register_rule(("Minion", 100))]
fn lt_to_ineq(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Lt(metadata, a, b) => Ok(Expr::Ineq(
            metadata.clone(),
            a.clone(),
            b.clone(),
            Box::new(Expr::Constant(Metadata::new(), Const::Int(-1))),
        )),
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
* Convert a Gt to an Ineq:
*
* ```text
* a > b => b - a < -1
* ```
*/
#[register_rule(("Minion", 100))]
fn gt_to_ineq(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Gt(metadata, a, b) => Ok(Expr::Ineq(
            metadata.clone(),
            b.clone(),
            a.clone(),
            Box::new(Expr::Constant(Metadata::new(), Const::Int(-1))),
        )),
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
* Convert a Geq to an Ineq:
*
* ```text
* a >= b => b - a < 0
* ```
*/
#[register_rule(("Minion", 100))]
fn geq_to_ineq(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Geq(metadata, a, b) => Ok(Expr::Ineq(
            metadata.clone(),
            b.clone(),
            a.clone(),
            Box::new(Expr::Constant(Metadata::new(), Const::Int(0))),
        )),
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
* Convert a Leq to an Ineq:
*
* ```text
* a <= b => a - b < 0
* ```
*/
#[register_rule(("Minion", 100))]
fn leq_to_ineq(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Leq(metadata, a, b) => Ok(Expr::Ineq(
            metadata.clone(),
            a.clone(),
            b.clone(),
            Box::new(Expr::Constant(Metadata::new(), Const::Int(0))),
        )),
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

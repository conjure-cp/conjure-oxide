use conjure_core::ast::Expression::Nothing;
use conjure_core::{
    ast::Constant as Const, ast::Expression as Expr, metadata::Metadata, rule::RuleApplicationError,
};
use conjure_rules::{register_rule, register_rule_set};
use uniplate::uniplate::Uniplate;

/*****************************************************************************/
/*        This file contains basic rules for simplifying expressions         */
/*****************************************************************************/

register_rule_set!("Base", 100, ());

/**
 * Remove nothing's from expressions:
 * ```text
 * and([a, nothing, b]) = and([a, b])
 * sum([a, nothing, b]) = sum([a, b])
 * sum_leq([a, nothing, b], c) = sum_leq([a, b], c)
 * ...
 * ```
*/
#[register_rule(("Base", 100))]
fn remove_nothings(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    fn remove_nothings(exprs: Vec<Expr>) -> Result<Vec<Expr>, RuleApplicationError> {
        let mut changed = false;
        let mut new_exprs = Vec::new();

        for e in exprs {
            match e.clone() {
                Nothing => {
                    changed = true;
                }
                _ => new_exprs.push(e),
            }
        }

        if changed {
            Ok(new_exprs)
        } else {
            Err(RuleApplicationError::RuleNotApplicable)
        }
    }

    fn get_lhs_rhs(sub: Vec<Expr>) -> (Vec<Expr>, Box<Expr>) {
        if sub.is_empty() {
            return (Vec::new(), Box::new(Nothing));
        }

        let lhs = sub[..(sub.len() - 1)].to_vec();
        let rhs = Box::new(sub[sub.len() - 1].clone());
        (lhs, rhs)
    }

    let new_sub = remove_nothings(expr.children())?;

    match expr {
        Expr::And(md, _) => Ok(Expr::And(md.clone(), new_sub)),
        Expr::Or(md, _) => Ok(Expr::Or(md.clone(), new_sub)),
        Expr::Sum(md, _) => Ok(Expr::Sum(md.clone(), new_sub)),
        Expr::SumEq(md, _, _) => {
            let (lhs, rhs) = get_lhs_rhs(new_sub);
            Ok(Expr::SumEq(md.clone(), lhs, rhs))
        }
        Expr::SumLeq(md, _lhs, _rhs) => {
            let (lhs, rhs) = get_lhs_rhs(new_sub);
            Ok(Expr::SumLeq(md.clone(), lhs, rhs))
        }
        Expr::SumGeq(md, _lhs, _rhs) => {
            let (lhs, rhs) = get_lhs_rhs(new_sub);
            Ok(Expr::SumGeq(md.clone(), lhs, rhs))
        }
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
 * Remove empty expressions:
 * ```text
 * [] = Nothing
 * ```
 */
#[register_rule(("Base", 100))]
fn empty_to_nothing(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Nothing | Expr::Reference(_, _) | Expr::Constant(_, _) => {
            Err(RuleApplicationError::RuleNotApplicable)
        }
        _ => {
            if expr.children().is_empty() {
                Ok(Nothing)
            } else {
                Err(RuleApplicationError::RuleNotApplicable)
            }
        }
    }
}

/**
 * Evaluate sum of constants:
 * ```text
 * sum([1, 2, 3]) = 6
 * ```
 */
#[register_rule(("Base", 100))]
fn sum_constants(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Sum(_, exprs) => {
            let mut sum = 0;
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Expr::Constant(_metadata, Const::Int(i)) => {
                        sum += i;
                        changed = true;
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(RuleApplicationError::RuleNotApplicable);
            }
            // TODO (kf77): Get existing metadata instead of creating a new one
            new_exprs.push(Expr::Constant(Metadata::new(), Const::Int(sum)));
            Ok(Expr::Sum(Metadata::new(), new_exprs)) // Let other rules handle only one Expr being contained in the sum
        }
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
 * Unwrap trivial sums:
 * ```text
 * sum([a]) = a
 * ```
 */
#[register_rule(("Base", 100))]
fn unwrap_sum(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Sum(_, exprs) if (exprs.len() == 1) => Ok(exprs[0].clone()),
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
 * Flatten nested sums:
 * ```text
 * sum(sum(a, b), c) = sum(a, b, c)
 * ```
 */
#[register_rule(("Base", 100))]
pub fn flatten_nested_sum(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Sum(metadata, exprs) => {
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Expr::Sum(_, sub_exprs) => {
                        changed = true;
                        for e in sub_exprs {
                            new_exprs.push(e.clone());
                        }
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(RuleApplicationError::RuleNotApplicable);
            }
            Ok(Expr::Sum(metadata.clone(), new_exprs))
        }
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/**
* Unwrap nested `or`

* ```text
* or(or(a, b), c) = or(a, b, c)
* ```
 */
#[register_rule(("Base", 100))]
fn unwrap_nested_or(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Or(metadata, exprs) => {
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Expr::Or(_, exprs) => {
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
            Ok(Expr::Or(metadata.clone(), new_exprs))
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
#[register_rule(("Base", 100))]
fn unwrap_nested_and(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::And(metadata, exprs) => {
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Expr::And(_, exprs) => {
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
            Ok(Expr::And(metadata.clone(), new_exprs))
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
#[register_rule(("Base", 100))]
fn remove_double_negation(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Not(_, contents) => match contents.as_ref() {
            Expr::Not(_, expr_box) => Ok(*expr_box.clone()),
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
#[register_rule(("Base", 100))]
fn remove_trivial_and(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::And(_, exprs) => {
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
#[register_rule(("Base", 100))]
fn remove_trivial_or(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Or(_, exprs) => {
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
#[register_rule(("Base", 100))]
fn remove_constants_from_or(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Or(metadata, exprs) => {
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Expr::Constant(metadata, Const::Bool(val)) => {
                        if *val {
                            // If we find a true, the whole expression is true
                            return Ok(Expr::Constant(metadata.clone(), Const::Bool(true)));
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
            Ok(Expr::Or(metadata.clone(), new_exprs))
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
#[register_rule(("Base", 100))]
fn remove_constants_from_and(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::And(metadata, exprs) => {
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Expr::Constant(metadata, Const::Bool(val)) => {
                        if !*val {
                            // If we find a false, the whole expression is false
                            return Ok(Expr::Constant(metadata.clone(), Const::Bool(false)));
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
            Ok(Expr::And(metadata.clone(), new_exprs))
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
#[register_rule(("Base", 100))]
fn evaluate_constant_not(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Not(_, contents) => match contents.as_ref() {
            Expr::Constant(metadata, Const::Bool(val)) => {
                Ok(Expr::Constant(metadata.clone(), Const::Bool(!val)))
            }
            _ => Err(RuleApplicationError::RuleNotApplicable),
        },
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/************************************************************************/
/*        Rules for translating to Minion-supported constraints         */
/************************************************************************/

use crate::ast::{
    Constant as Const, DecisionVariable, Domain, Expression as Expr, Range, SymbolTable,
};
use crate::metadata::Metadata;
use crate::rule_engine::{
    register_rule, register_rule_set, ApplicationError, ApplicationResult, Reduction,
};
use crate::solver::SolverFamily;
use crate::Model;
use uniplate::biplate::Uniplate;

register_rule_set!("Minion", 100, ("Base"), (SolverFamily::Minion));

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
fn sum_to_vector(expr: &Expr) -> Result<Vec<Expr>, ApplicationError> {
    match expr {
        Expr::Sum(_, exprs) => {
            if is_nested_sum(exprs) {
                Err(ApplicationError::RuleNotApplicable)
            } else {
                Ok(exprs.clone())
            }
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

// /**
//  * Convert an Eq to a conjunction of Geq and Leq:
//  * ```text
//  * a = b => a >= b && a <= b
//  * ```
//  */
// #[register_rule(("Minion", 100))]
// fn eq_to_minion(expr: &Expr, _: &Model) -> ApplicationResult {
//     match expr {
//         Expr::Eq(metadata, a, b) => Ok(Reduction::pure(Expr::And(
//             metadata.clone_dirty(),
//             vec![
//                 Expr::Geq(metadata.clone_dirty(), a.clone(), b.clone()),
//                 Expr::Leq(metadata.clone_dirty(), a.clone(), b.clone()),
//             ],
//         ))),
//         _ => Err(ApplicationError::RuleNotApplicable),
//     }
// }

/**
 * Convert a Geq to a SumGeq if the left hand side is a sum:
 * ```text
 * sum([a, b, c]) >= d => sum_geq([a, b, c], d)
 * ```
 */
#[register_rule(("Minion", 100))]
fn flatten_sum_geq(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Geq(metadata, a, b) => {
            let exprs = sum_to_vector(a)?;
            Ok(Reduction::pure(Expr::SumGeq(
                metadata.clone_dirty(),
                exprs,
                b.clone(),
            )))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Convert a Leq to a SumLeq if the left hand side is a sum:
 * ```text
 * sum([a, b, c]) <= d => sum_leq([a, b, c], d)
 * ```
 */
#[register_rule(("Minion", 100))]
fn sum_leq_to_sumleq(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Leq(metadata, a, b) => {
            let exprs = sum_to_vector(a)?;
            Ok(Reduction::pure(Expr::SumLeq(
                metadata.clone_dirty(),
                exprs,
                b.clone(),
            )))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Convert a 'Eq(Sum([...]))' to a SumEq
 * ```text
 * eq(sum([a, b]), c) => sumeq([a, b], c)
 * ```
*/
#[register_rule(("Minion", 100))]
fn sum_eq_to_sumeq(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Eq(metadata, a, b) => {
            if let Ok(exprs) = sum_to_vector(a) {
                Ok(Reduction::pure(Expr::SumEq(
                    metadata.clone_dirty(),
                    exprs,
                    b.clone(),
                )))
            } else if let Ok(exprs) = sum_to_vector(b) {
                Ok(Reduction::pure(Expr::SumEq(
                    metadata.clone_dirty(),
                    exprs,
                    a.clone(),
                )))
            } else {
                Err(ApplicationError::RuleNotApplicable)
            }
        }
        _ => Err(ApplicationError::RuleNotApplicable),
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
fn sumeq_to_minion(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::SumEq(metadata, exprs, eq_to) => Ok(Reduction::pure(Expr::And(
            Metadata::new(),
            vec![
                Expr::SumGeq(Metadata::new(), exprs.clone(), Box::from(*eq_to.clone())),
                Expr::SumLeq(Metadata::new(), exprs.clone(), Box::from(*eq_to.clone())),
            ],
        ))),
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
* Convert a Lt to an Ineq:

* ```text
* a < b => a - b < -1
* ```
*/
#[register_rule(("Minion", 100))]
fn lt_to_ineq(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Lt(metadata, a, b) => Ok(Reduction::pure(Expr::Ineq(
            metadata.clone_dirty(),
            a.clone(),
            b.clone(),
            Box::new(Expr::Constant(Metadata::new(), Const::Int(-1))),
        ))),
        _ => Err(ApplicationError::RuleNotApplicable),
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
fn gt_to_ineq(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Gt(metadata, a, b) => Ok(Reduction::pure(Expr::Ineq(
            metadata.clone_dirty(),
            b.clone(),
            a.clone(),
            Box::new(Expr::Constant(Metadata::new(), Const::Int(-1))),
        ))),
        _ => Err(ApplicationError::RuleNotApplicable),
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
fn geq_to_ineq(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Geq(metadata, a, b) => Ok(Reduction::pure(Expr::Ineq(
            metadata.clone_dirty(),
            b.clone(),
            a.clone(),
            Box::new(Expr::Constant(Metadata::new(), Const::Int(0))),
        ))),
        _ => Err(ApplicationError::RuleNotApplicable),
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
fn leq_to_ineq(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Leq(metadata, a, b) => Ok(Reduction::pure(Expr::Ineq(
            metadata.clone_dirty(),
            a.clone(),
            b.clone(),
            Box::new(Expr::Constant(Metadata::new(), Const::Int(0))),
        ))),
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

// #[register_rule(("Minion", 99))]
// fn eq_to_leq_geq(expr: &Expr, _: &Model) -> ApplicationResult {
//     match expr {
//         Expr::Eq(metadata, a, b) => {
//             return Ok(Reduction::pure(Expr::And(
//                 metadata.clone(),
//                 vec![
//                     Expr::Leq(metadata.clone(), a.clone(), b.clone()),
//                     Expr::Geq(metadata.clone(), a.clone(), b.clone()),
//                 ],
//             )));
//         }
//         _ => Err(ApplicationError::RuleNotApplicable),
//     }
// }

/**
 * Since Minion doesn't support some constraints with div (e.g. leq, neq), we add an auxiliary variable to represent the division result.
*/
#[register_rule(("Minion", 101))]
fn flatten_safediv(expr: &Expr, mdl: &Model) -> ApplicationResult {
    if expr.is_eq() || expr.is_leq() || expr.is_geq() || expr.is_neq() {
        let mut sub = expr.children();

        let mut new_vars = SymbolTable::new();
        let mut new_top = vec![];

        // replace every safe div child with a reference to a new variable
        for c in sub.iter_mut() {
            if let Expr::SafeDiv(_, a, b) = c.clone() {
                let new_name = mdl.gensym();
                let domain = c
                    .domain_of(&mdl.variables)
                    .ok_or(ApplicationError::DomainError)?;
                new_vars.insert(new_name.clone(), DecisionVariable::new(domain));

                new_top.push(Expr::DivEq(
                    Metadata::new(),
                    a.clone(),
                    b.clone(),
                    Box::new(Expr::Reference(Metadata::new(), new_name.clone())),
                ));

                *c = Expr::Reference(Metadata::new(), new_name.clone());
            }
        }
        if !new_top.is_empty() {
            return Ok(Reduction::new(
                expr.with_children(sub),
                Expr::And(Metadata::new(), new_top),
                new_vars,
            ));
        }
    }
    Err(ApplicationError::RuleNotApplicable)
}

#[register_rule(("Minion", 100))]
fn div_eq_to_diveq(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Eq(metadata, a, b) => {
            if let Expr::SafeDiv(_, x, y) = a.as_ref() {
                if !(b.is_reference() || b.is_constant()) {
                    return Err(ApplicationError::RuleNotApplicable);
                }
                Ok(Reduction::pure(Expr::DivEq(
                    metadata.clone_dirty(),
                    x.clone(),
                    y.clone(),
                    b.clone(),
                )))
            } else if let Expr::SafeDiv(_, x, y) = b.as_ref() {
                if !(a.is_reference() || a.is_constant()) {
                    return Err(ApplicationError::RuleNotApplicable);
                }
                Ok(Reduction::pure(Expr::DivEq(
                    metadata.clone_dirty(),
                    x.clone(),
                    y.clone(),
                    a.clone(),
                )))
            } else {
                Err(ApplicationError::RuleNotApplicable)
            }
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

#[register_rule(("Minion", 100))]
fn negated_neq_to_eq(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Not(_, a) => match a.as_ref() {
            Expr::Neq(_, b, c) => {
                if !b.can_be_undefined() && !c.can_be_undefined() {
                    Ok(Reduction::pure(Expr::Eq(
                        Metadata::new(),
                        b.clone(),
                        c.clone(),
                    )))
                } else {
                    Err(ApplicationError::RuleNotApplicable)
                }
            }
            _ => Err(ApplicationError::RuleNotApplicable),
        },
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

#[register_rule(("Minion", 100))]
fn negated_eq_to_neq(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Not(_, a) => match a.as_ref() {
            Expr::Eq(_, b, c) => {
                if !b.can_be_undefined() && !c.can_be_undefined() {
                    Ok(Reduction::pure(Expr::Neq(
                        Metadata::new(),
                        b.clone(),
                        c.clone(),
                    )))
                } else {
                    Err(ApplicationError::RuleNotApplicable)
                }
            }
            _ => Err(ApplicationError::RuleNotApplicable),
        },
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

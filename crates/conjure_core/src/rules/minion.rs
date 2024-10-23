/************************************************************************/
/*        Rules for translating to Minion-supported constraints         */
/************************************************************************/

use crate::ast::{Constant as Const, DecisionVariable, Domain, Expression as Expr, SymbolTable};
use crate::metadata::Metadata;
use crate::rule_engine::{
    register_rule, register_rule_set, ApplicationError, ApplicationResult, Reduction,
};

use crate::solver::SolverFamily;
use crate::Model;
use uniplate::Uniplate;
use ApplicationError::RuleNotApplicable;

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
#[register_rule(("Minion", 4400))]
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
#[register_rule(("Minion", 4400))]
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
#[register_rule(("Minion", 4400))]
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
#[register_rule(("Minion", 4400))]
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
#[register_rule(("Minion", 4100))]
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
#[register_rule(("Minion", 4100))]
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
#[register_rule(("Minion", 4100))]
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
#[register_rule(("Minion", 4100))]
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

/// ```text
/// x <= y + k ~> ineq(x,y,k)
/// ```

#[register_rule(("Minion",4400))]
fn x_leq_y_plus_k_to_ineq(expr: &Expr, _: &Model) -> ApplicationResult {
    let Expr::Leq(_, x, b) = expr else {
        return Err(ApplicationError::RuleNotApplicable);
    };

    let x @ Expr::Reference(_, _) = *x.to_owned() else {
        return Err(ApplicationError::RuleNotApplicable);
    };

    let Expr::Sum(_, c) = *b.to_owned() else {
        return Err(ApplicationError::RuleNotApplicable);
    };

    let [ref y @ Expr::Reference(_, _), ref k @ Expr::Constant(_, _)] = c[..] else {
        return Err(ApplicationError::RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::Ineq(
        expr.get_meta().clone_dirty(),
        Box::new(x),
        Box::new(y.clone()),
        Box::new(k.clone()),
    )))
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
#[register_rule(("Minion", 4400))]
fn flatten_safediv(expr: &Expr, mdl: &Model) -> ApplicationResult {
    use Expr::*;
    match expr {
        Eq(_, _, _) => {}
        Leq(_, _, _) => {}
        Geq(_, _, _) => {}
        Neq(_, _, _) => {}
        _ => {
            return Err(ApplicationError::RuleNotApplicable);
        }
    }

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
    Err(ApplicationError::RuleNotApplicable)
}

#[register_rule(("Minion", 4400))]
fn div_eq_to_diveq(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Expr::Eq(metadata, a, b) => {
            if let Expr::SafeDiv(_, x, y) = a.as_ref() {
                match **b {
                    Expr::Reference(_, _) | Expr::Constant(_, _) => {}
                    _ => {
                        return Err(ApplicationError::RuleNotApplicable);
                    }
                };

                Ok(Reduction::pure(Expr::DivEq(
                    metadata.clone_dirty(),
                    x.clone(),
                    y.clone(),
                    b.clone(),
                )))
            } else if let Expr::SafeDiv(_, x, y) = b.as_ref() {
                match **a {
                    Expr::Reference(_, _) | Expr::Constant(_, _) => {}
                    _ => {
                        return Err(ApplicationError::RuleNotApplicable);
                    }
                };
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

#[register_rule(("Minion", 4400))]
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

#[register_rule(("Minion", 4400))]
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

/// Flattening rule that converts boolean variables to watched-literal constraints.
///
/// For some boolean variable x:
/// ```text
/// and([x,...]) ~> and([w-literal(x,1),..])
///  or([x,...]) ~>  or([w-literal(x,1),..])
///  not(x)      ~>  w-literal(x,0)
/// ```
///
/// ## Rationale
///
/// Minion's watched-and and watched-or constraints only takes other constraints as arguments.
///
/// This restates boolean variables as the equivalent constraint "SAT if x is true".
///
#[register_rule(("Minion", 4100))]
fn boolean_literal_to_wliteral(expr: &Expr, mdl: &Model) -> ApplicationResult {
    use Domain::BoolDomain;
    use Expr::*;
    match expr {
        Or(m, vec) => {
            let mut changed = false;
            let mut new_vec = Vec::new();
            for expr in vec {
                new_vec.push(match expr {
                    Reference(m, name)
                        if mdl
                            .get_domain(name)
                            .is_some_and(|x| matches!(x, BoolDomain)) =>
                    {
                        changed = true;
                        WatchedLiteral(m.clone_dirty(), name.clone(), Const::Bool(true))
                    }
                    e => e.clone(),
                });
            }

            if !changed {
                return Err(RuleNotApplicable);
            }

            Ok(Reduction::pure(Or(m.clone_dirty(), new_vec)))
        }
        And(m, vec) => {
            let mut changed = false;
            let mut new_vec = Vec::new();
            for expr in vec {
                new_vec.push(match expr {
                    Reference(m, name)
                        if mdl
                            .get_domain(name)
                            .is_some_and(|x| matches!(x, BoolDomain)) =>
                    {
                        changed = true;
                        WatchedLiteral(m.clone_dirty(), name.clone(), Const::Bool(true))
                    }
                    e => e.clone(),
                });
            }

            if !changed {
                return Err(RuleNotApplicable);
            }

            Ok(Reduction::pure(And(m.clone_dirty(), new_vec)))
        }

        Not(m, expr) => {
            if let Reference(_, name) = (**expr).clone() {
                if mdl
                    .get_domain(&name)
                    .is_some_and(|x| matches!(x, BoolDomain))
                {
                    return Ok(Reduction::pure(WatchedLiteral(
                        m.clone_dirty(),
                        name.clone(),
                        Const::Bool(false),
                    )));
                }
            }
            Err(RuleNotApplicable)
        }
        _ => Err(RuleNotApplicable),
    }
}

/// Flattening rule for not(X) in Minion, where X is a constraint.
///
/// ```text
/// not(X) ~> reify(X,0)
/// ```
///
/// This rule has lower priority than boolean_literal_to_wliteral so that we can assume that the
/// nested expressions are constraints not variables.

#[register_rule(("Minion", 4090))]
fn not_constraint_to_reify(expr: &Expr, _: &Model) -> ApplicationResult {
    use Expr::*;
    if !matches!(expr, Not(_,c) if !matches!(**c, Reference(_,_)|Constant(_,_))) {
        return Err(RuleNotApplicable);
    }

    let Not(m, e) = expr else {
        unreachable!();
    };

    Ok(Reduction::pure(Reify(
        m.clone(),
        e.clone(),
        Box::new(Constant(Metadata::new(), Const::Bool(false))),
    )))
}

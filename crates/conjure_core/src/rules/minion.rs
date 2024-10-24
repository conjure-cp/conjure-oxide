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
fn sumgeq_introduction(expr: &Expr, _: &Model) -> ApplicationResult {
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
        Expr::SumEq(_, exprs, eq_to) => Ok(Reduction::pure(Expr::And(
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
* a < b ~> a <= b -1 ~> ineq(a,b,-1)
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
* a > b ~> b <= a -1 ~> ineq(b,a,-1)
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
* a >= b ~> b <= a + 0 ~> ineq(b,a,0)
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
* a <= b ~> a <= b + 0 ~> ineq(a,b,0)
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

#[register_rule(("Minion", 4400))]
fn div_to_diveq(expr: &Expr, m: &Model) -> ApplicationResult {
    match expr {
        Expr::Eq(metadata, a, b) => {
            if let Expr::SafeDiv(_, x, y) = a.as_ref() {
                // first put things into minion native constraints, then flatten.
                // so no checks for constants, expressions,etc.
                Ok(Reduction::pure(Expr::DivEq(
                    metadata.clone_dirty(),
                    x.clone(),
                    y.clone(),
                    b.clone(),
                )))
            } else if let Expr::SafeDiv(_, x, y) = b.as_ref() {
                // flattening later on checks if this is ref or constant.
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
        // must be in form x/y = z
        // x/y ~> z, x/y=z
        Expr::SafeDiv(_, x, y) => {
            let mut m = m.clone();
            let aux_var_domain = expr.domain_of(&m.variables).expect(&format!(
                "expr.domain_of() failed in Minion div_to_diveq for {:#?}",
                expr
            ));

            let aux_var_name = m.gensym();
            m.add_variable(aux_var_name.clone(), DecisionVariable::new(aux_var_domain));

            let new_expr = Expr::Reference(Metadata::new(), aux_var_name);
            let new_top = Expr::DivEq(
                Metadata::new(),
                x.clone(),
                y.clone(),
                Box::new(new_expr.clone()),
            );
            Ok(Reduction::new(new_expr, new_top, m.variables))
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

/// Flatten binary constraints by introducing an auxiliary variable.
///
/// For example:
///
/// ```text
///  a / (b/q) = d ~>
///     x=b/q,
///     a/x = d
/// ```
///
/// This rule only applies when b and q are constants or references, as the posted constraint must
/// also be flat. This, in effect, forces bottom-up flattening.
///
/// Note that we first turn expressions into arithmetic constraints (i.e. diveq not eq(safediv)), then flatten.
#[register_rule(("Minion",4100))]
fn flatten_binary_operators(expr: &Expr, m: &Model) -> ApplicationResult {
    if !(matches!(expr, Expr::DivEq(_, _, _, _))) {
        return Err(RuleNotApplicable);
    }

    let subexprs = expr.children();

    // assuming binary operator = x
    assert_eq!(subexprs.len(), 3);

    if !subexprs
        .iter()
        .all(|x| matches!(x, Expr::Constant(_, _) | Expr::Reference(_, _)))
    {
        return Err(RuleNotApplicable);
    };

    let aux_var_domain = expr.domain_of(&m.variables).expect(&format!(
        "expr.domain_of() failed in Minion flatten_arithmetic for {:#?}",
        expr
    ));

    let aux_name = m.gensym();

    let mut m = m.clone();
    m.add_variable(aux_name.clone(), DecisionVariable::new(aux_var_domain));

    let new_expr = Expr::Reference(Metadata::new(), aux_name.clone());

    // a `op` b = new_var
    let new_top =
        expr.with_children(vec![subexprs[0].clone(), subexprs[1].clone(), new_expr.clone()].into());

    Ok(Reduction::new(new_expr, new_top, m.variables))
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

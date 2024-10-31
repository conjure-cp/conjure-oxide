/************************************************************************/
/*        Rules for translating to Minion-supported constraints         */
/************************************************************************/

use crate::ast::{Constant as Const, DecisionVariable, Domain, Expression as Expr, SymbolTable};
use crate::metadata::Metadata;
use crate::rule_engine::{
    register_rule, register_rule_set, ApplicationError, ApplicationResult, Reduction,
};

use crate::solver::SolverFamily;
use crate::{bug, Model};
use itertools::Itertools;
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

#[register_rule(("Minion", 4400))]
fn flatten_binops(expr: &Expr, m: &Model) -> ApplicationResult {
    use Expr::*;
    if !matches!(expr, SafeDiv(_, _, _)) {
        return Err(RuleNotApplicable);
    }

    // from the above guard, assuming that expr is a binop.
    assert_eq!(expr.children().len(), 2);
    let mut a = expr.children()[0].clone();
    let mut b = expr.children()[1].clone();

    let mut new_top_level_constraints: Vec<Expr> = vec![];
    let mut m1 = m.clone();

    // simplify lhs and rhs expressions.
    if let Some((a1, a_top)) = expr_to_aux_var(&a, &mut m1) {
        new_top_level_constraints.push(a_top);
        a = a1;
    };

    if let Some((b1, b_top)) = expr_to_aux_var(&b, &mut m1) {
        new_top_level_constraints.push(b_top);
        b = b1;
    };

    let new_top = match new_top_level_constraints.as_slice() {
        [] => {
            return Err(RuleNotApplicable);
        } // no change to expr
        [a] => a.clone(),
        [_, _] => Expr::And(Metadata::new(), new_top_level_constraints),
        _ => unreachable!(),
    };

    let new_expr = expr.with_children(vec![a, b].into_iter().collect());
    Ok(Reduction::new(new_expr, new_top, m1.variables))
}

/// Saves the given expression into a new auxiliary variable.
///
/// The model is mutated with the auxiliary variable. A tuple of the replacement expression (of the form
/// Expr::Reference(...)), and a new top level constraint are returned.
///
/// If the expression is already a constant or a reference, None is returned.
fn expr_to_aux_var(expr: &Expr, m: &mut Model) -> Option<(Expr, Expr)> {
    if matches!(expr, Expr::Constant(_, _) | Expr::Reference(_, _)) {
        return None;
    }

    let name = m.gensym();
    let Some(domain) = expr.domain_of(&m.variables) else {
        bug!("rules/minion.rs:expr_to_aux_var: Could not find domain of {expr}")
    };

    m.add_variable(name.clone(), DecisionVariable::new(domain));

    let new_expr = Expr::Reference(Metadata::new(), name);
    let new_top = Expr::Eq(
        Metadata::new(),
        Box::new(new_expr.clone()),
        Box::new(expr.clone()),
    );

    Some((new_expr, new_top))
}

/// Returns true iff the children of `expr` are not nested expressions.
fn is_flattened(expr: &Expr) -> bool {
    use itertools::FoldWhile::{Continue, Done};

    expr.children()
        .into_iter()
        .fold_while(true, |_, x| {
            if matches!(x, Expr::Constant(_, _) | Expr::Reference(_, _)) {
                Continue(true)
            } else {
                Done(false)
            }
        })
        .into_inner()
}

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
        Expr::SumEq(_metadata, exprs, eq_to) => Ok(Reduction::pure(Expr::And(
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

#[register_rule(("Minion", 4400))]
fn div_eq_to_diveq(expr: &Expr, _: &Model) -> ApplicationResult {
    use Expr::*;

    let negated = match expr {
        Eq(_, _, _) => false,
        Neq(_, _, _) => true,
        _ => return Err(RuleNotApplicable),
    };

    // binary operators
    assert_eq!(expr.children().len(), 2);

    let metadata = expr.get_meta();
    let a = expr.children()[0].clone();
    let b = expr.children()[1].clone();

    if let Expr::SafeDiv(_, x, y) = a.clone() {
        if !is_flattened(&a) {
            return Err(RuleNotApplicable);
        }
        match b {
            Reference(_, _) | Constant(_, _) => {}
            _ => {
                return Err(ApplicationError::RuleNotApplicable);
            }
        };

        if negated {
            return Ok(Reduction::pure(Not(
                metadata.clone_dirty(),
                Box::new(DivEq(
                    Metadata::new(),
                    x.clone(),
                    y.clone(),
                    Box::new(b.clone()),
                )),
            )));
        } else {
            return Ok(Reduction::pure(Expr::DivEq(
                metadata.clone_dirty(),
                x.clone(),
                y.clone(),
                Box::new(b.clone()),
            )));
        }
    } else if let Expr::SafeDiv(_, x, y) = b.clone() {
        if !is_flattened(&b) {
            return Err(RuleNotApplicable);
        }
        match a {
            Expr::Reference(_, _) | Expr::Constant(_, _) => {}
            _ => {
                return Err(ApplicationError::RuleNotApplicable);
            }
        };

        if negated {
            return Ok(Reduction::pure(Not(
                metadata.clone_dirty(),
                Box::new(DivEq(
                    Metadata::new(),
                    x.clone(),
                    y.clone(),
                    Box::new(a.clone()),
                )),
            )));
        } else {
            return Ok(Reduction::pure(Expr::DivEq(
                metadata.clone_dirty(),
                x.clone(),
                y.clone(),
                Box::new(a.clone()),
            )));
        }
    } else {
        Err(ApplicationError::RuleNotApplicable)
    }
}

#[register_rule(("Minion", 4200))]
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

#[register_rule(("Minion", 4200))]
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

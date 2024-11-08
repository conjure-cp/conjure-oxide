/************************************************************************/
/*        Rules for translating to Minion-supported constraints         */
/************************************************************************/

use crate::ast::{
    DecisionVariable, Domain, Expression as Expr, Expression::*, Factor::*, Literal::*, SymbolTable,
};
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
        if let Sum(_, _) = e {
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
        Sum(_, exprs) => {
            if is_nested_sum(exprs) {
                Err(RuleNotApplicable)
            } else {
                Ok(exprs.clone())
            }
        }
        _ => Err(RuleNotApplicable),
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
        Geq(metadata, a, b) => {
            let exprs = sum_to_vector(a)?;
            Ok(Reduction::pure(SumGeq(
                metadata.clone_dirty(),
                exprs,
                b.clone(),
            )))
        }
        _ => Err(RuleNotApplicable),
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
        Leq(metadata, a, b) => {
            let exprs = sum_to_vector(a)?;
            Ok(Reduction::pure(SumLeq(
                metadata.clone_dirty(),
                exprs,
                b.clone(),
            )))
        }
        _ => Err(RuleNotApplicable),
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
        Eq(metadata, a, b) => {
            if let Ok(exprs) = sum_to_vector(a) {
                Ok(Reduction::pure(SumEq(
                    metadata.clone_dirty(),
                    exprs,
                    b.clone(),
                )))
            } else if let Ok(exprs) = sum_to_vector(b) {
                Ok(Reduction::pure(SumEq(
                    metadata.clone_dirty(),
                    exprs,
                    a.clone(),
                )))
            } else {
                Err(RuleNotApplicable)
            }
        }
        _ => Err(RuleNotApplicable),
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
        SumEq(_metadata, exprs, eq_to) => Ok(Reduction::pure(And(
            Metadata::new(),
            vec![
                SumGeq(Metadata::new(), exprs.clone(), Box::from(*eq_to.clone())),
                SumLeq(Metadata::new(), exprs.clone(), Box::from(*eq_to.clone())),
            ],
        ))),
        _ => Err(RuleNotApplicable),
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
        Lt(metadata, a, b) => Ok(Reduction::pure(Ineq(
            metadata.clone_dirty(),
            a.clone(),
            b.clone(),
            Box::new(FactorE(Metadata::new(), Literal(Int(-1)))),
        ))),
        _ => Err(RuleNotApplicable),
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
        Gt(metadata, a, b) => Ok(Reduction::pure(Ineq(
            metadata.clone_dirty(),
            b.clone(),
            a.clone(),
            Box::new(FactorE(Metadata::new(), Literal(Int(-1)))),
        ))),
        _ => Err(RuleNotApplicable),
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
        Geq(metadata, a, b) => Ok(Reduction::pure(Ineq(
            metadata.clone_dirty(),
            b.clone(),
            a.clone(),
            Box::new(FactorE(Metadata::new(), Literal(Int(0)))),
        ))),
        _ => Err(RuleNotApplicable),
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
        Leq(metadata, a, b) => Ok(Reduction::pure(Ineq(
            metadata.clone_dirty(),
            a.clone(),
            b.clone(),
            Box::new(FactorE(Metadata::new(), Literal(Int(0)))),
        ))),
        _ => Err(RuleNotApplicable),
    }
}

/// ```text
/// x <= y + k ~> ineq(x,y,k)
/// ```

#[register_rule(("Minion",4400))]
fn x_leq_y_plus_k_to_ineq(expr: &Expr, _: &Model) -> ApplicationResult {
    let Leq(_, x, b) = expr else {
        return Err(RuleNotApplicable);
    };

    let x @ FactorE(_, Reference(_)) = *x.to_owned() else {
        return Err(RuleNotApplicable);
    };

    let Sum(_, c) = *b.to_owned() else {
        return Err(RuleNotApplicable);
    };

    let [ref y @ FactorE(_, Reference(_)), ref k @ FactorE(_, Literal(_))] = c[..] else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Ineq(
        expr.get_meta().clone_dirty(),
        Box::new(x),
        Box::new(y.clone()),
        Box::new(k.clone()),
    )))
}

// #[register_rule(("Minion", 99))]
// fn eq_to_leq_geq(expr: &Expr, _: &Model) -> ApplicationResult {
//     match expr {
//         Eq(metadata, a, b) => {
//             return Ok(Reduction::pure(Expr::And(
//                 metadata.clone(),
//                 vec![
//                     Expr::Leq(metadata.clone(), a.clone(), b.clone()),
//                     Expr::Geq(metadata.clone(), a.clone(), b.clone()),
//                 ],
//             )));
//         }
//         _ => Err(RuleNotApplicable),
//     }
// }

/**
 * Since Minion doesn't support some constraints with div (e.g. leq, neq), we add an auxiliary variable to represent the division result.
*/
#[register_rule(("Minion", 4400))]
fn flatten_safediv(expr: &Expr, mdl: &Model) -> ApplicationResult {
    match expr {
        Eq(_, _, _) => {}
        Leq(_, _, _) => {}
        Geq(_, _, _) => {}
        Neq(_, _, _) => {}
        _ => {
            return Err(RuleNotApplicable);
        }
    }

    let mut sub = expr.children();

    let mut new_vars = SymbolTable::new();
    let mut new_top = vec![];

    // replace every safe div child with a reference to a new variable
    let mut num_changed = 0;
    for c in sub.iter_mut() {
        if let SafeDiv(_, a, b) = c.clone() {
            num_changed += 1;
            let new_name = mdl.gensym();
            let domain = c
                .domain_of(&mdl.variables)
                .ok_or(ApplicationError::DomainError)?;
            new_vars.insert(new_name.clone(), DecisionVariable::new(domain));

            new_top.push(DivEq(
                Metadata::new(),
                a.clone(),
                b.clone(),
                Box::new(FactorE(Metadata::new(), Reference(new_name.clone()))),
            ));

            *c = FactorE(Metadata::new(), Reference(new_name.clone()));
        }
    }

    //  want to turn Eq(a/b,c) into DivEq(a,b,c) instead, so this rule doesn't apply!
    if num_changed <= 1 && matches!(expr, Eq(_, _, _) | Neq(_, _, _)) {
        return Err(RuleNotApplicable);
    }

    if !new_top.is_empty() {
        return Ok(Reduction::new(
            expr.with_children(sub),
            And(Metadata::new(), new_top),
            new_vars,
        ));
    }
    Err(RuleNotApplicable)
}

#[register_rule(("Minion", 4400))]
fn div_eq_to_diveq(expr: &Expr, _: &Model) -> ApplicationResult {
    let negated = match expr {
        Eq(_, _, _) => false,
        Neq(_, _, _) => true,
        _ => {
            return Err(RuleNotApplicable);
        }
    };
    let metadata = expr.get_meta();
    let a = expr.children()[0].clone();
    let b = expr.children()[1].clone();

    if let SafeDiv(_, x, y) = a {
        match b {
            FactorE(_, _) => {}
            _ => {
                return Err(RuleNotApplicable);
            }
        };

        if negated {
            Ok(Reduction::pure(Not(
                metadata.clone_dirty(),
                Box::new(DivEq(
                    Metadata::new(),
                    x.clone(),
                    y.clone(),
                    Box::new(b.clone()),
                )),
            )))
        } else {
            Ok(Reduction::pure(DivEq(
                metadata.clone_dirty(),
                x.clone(),
                y.clone(),
                Box::new(b.clone()),
            )))
        }
    } else if let SafeDiv(_, x, y) = b {
        match a {
            FactorE(_, _) => {}
            _ => {
                return Err(RuleNotApplicable);
            }
        };

        if negated {
            Ok(Reduction::pure(Not(
                metadata.clone_dirty(),
                Box::new(DivEq(
                    Metadata::new(),
                    x.clone(),
                    y.clone(),
                    Box::new(a.clone()),
                )),
            )))
        } else {
            Ok(Reduction::pure(DivEq(
                metadata.clone_dirty(),
                x.clone(),
                y.clone(),
                Box::new(a.clone()),
            )))
        }
    } else {
        Err(RuleNotApplicable)
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
    match expr {
        Or(m, vec) => {
            let mut changed = false;
            let mut new_vec = Vec::new();
            for expr in vec {
                new_vec.push(match expr {
                    FactorE(m, Reference(name))
                        if mdl
                            .get_domain(name)
                            .is_some_and(|x| matches!(x, BoolDomain)) =>
                    {
                        changed = true;
                        WatchedLiteral(m.clone_dirty(), name.clone(), Bool(true))
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
                    FactorE(m, Reference(name))
                        if mdl
                            .get_domain(name)
                            .is_some_and(|x| matches!(x, BoolDomain)) =>
                    {
                        changed = true;
                        WatchedLiteral(m.clone_dirty(), name.clone(), Bool(true))
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
            if let FactorE(_, Reference(name)) = (**expr).clone() {
                if mdl
                    .get_domain(&name)
                    .is_some_and(|x| matches!(x, BoolDomain))
                {
                    return Ok(Reduction::pure(WatchedLiteral(
                        m.clone_dirty(),
                        name.clone(),
                        Bool(false),
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
    if !matches!(expr, Not(_,c) if !matches!(**c, FactorE(_,_))) {
        return Err(RuleNotApplicable);
    }

    let Not(m, e) = expr else {
        unreachable!();
    };

    Ok(Reduction::pure(Reify(
        m.clone(),
        e.clone(),
        Box::new(FactorE(Metadata::new(), Literal(Bool(false)))),
    )))
}

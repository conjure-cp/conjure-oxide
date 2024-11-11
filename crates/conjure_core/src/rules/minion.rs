/************************************************************************/
/*        Rules for translating to Minion-supported constraints         */
/************************************************************************/

use crate::ast::{
    Atom::{self, *},
    Domain,
    Expression::{self as Expr, *},
    Literal::*,
};
use crate::metadata::Metadata;
use crate::rule_engine::{
    register_rule, register_rule_set, ApplicationError, ApplicationResult, Reduction,
};
use crate::rules::extra_check;

use crate::solver::SolverFamily;
use crate::Model;
use uniplate::Uniplate;
use ApplicationError::RuleNotApplicable;

use super::utils::{is_flat, to_aux_var};

register_rule_set!("Minion", 100, ("Base"), (SolverFamily::Minion));

#[register_rule(("Minion", 4200))]
fn introduce_diveq(expr: &Expr, _: &Model) -> ApplicationResult {
    // div = val
    let val: Atom;
    let div: Expr;
    let meta: Metadata;

    match expr.clone() {
        Expr::Eq(m, a, b) => {
            meta = m;
            if let Some(f) = a.as_atom() {
                // val = div
                val = f;
                div = *b;
            } else if let Some(f) = b.as_atom() {
                // div = val
                val = f;
                div = *a;
            } else {
                return Err(RuleNotApplicable);
            }
        }
        Expr::AuxDeclaration(m, name, e) => {
            meta = m;
            val = name.into();
            div = *e;
        }
        _ => {
            return Err(RuleNotApplicable);
        }
    }

    if !(matches!(div, Expr::SafeDiv(_, _, _))) {
        return Err(RuleNotApplicable);
    }

    let children = div.children();
    let a = children[0].as_atom().ok_or(RuleNotApplicable)?;
    let b = children[1].as_atom().ok_or(RuleNotApplicable)?;

    Ok(Reduction::pure(DivEqUndefZero(
        meta.clone_dirty(),
        a,
        b,
        val,
    )))
}

#[register_rule(("Minion", 4200))]
fn introduce_modeq(expr: &Expr, _: &Model) -> ApplicationResult {
    // div = val
    let val: Atom;
    let div: Expr;
    let meta: Metadata;

    match expr.clone() {
        Expr::Eq(m, a, b) => {
            meta = m;
            if let Some(f) = a.as_atom() {
                // val = div
                val = f;
                div = *b;
            } else if let Some(f) = b.as_atom() {
                // div = val
                val = f;
                div = *a;
            } else {
                return Err(RuleNotApplicable);
            }
        }
        Expr::AuxDeclaration(m, name, e) => {
            meta = m;
            val = name.into();
            div = *e;
        }
        _ => {
            return Err(RuleNotApplicable);
        }
    }

    if !(matches!(div, Expr::SafeMod(_, _, _))) {
        return Err(RuleNotApplicable);
    }

    let children = div.children();
    let a = children[0].as_atom().ok_or(RuleNotApplicable)?;
    let b = children[1].as_atom().ok_or(RuleNotApplicable)?;

    Ok(Reduction::pure(ModuloEqUndefZero(
        meta.clone_dirty(),
        a,
        b,
        val,
    )))
}

#[register_rule(("Minion", 4400))]
fn flatten_binop(expr: &Expr, model: &Model) -> ApplicationResult {
    if !matches!(
        expr,
        Expr::SafeDiv(_, _, _) | Expr::Neq(_, _, _) | Expr::SafeMod(_, _, _)
    ) {
        return Err(RuleNotApplicable);
    }

    let mut children = expr.children();
    debug_assert_eq!(children.len(), 2);

    let mut model = model.clone();
    let mut num_changed = 0;
    let mut new_tops: Vec<Expr> = vec![];

    for child in children.iter_mut() {
        if let Some(aux_var_info) = to_aux_var(child, &model) {
            model = aux_var_info.model();
            new_tops.push(aux_var_info.top_level_expr());
            *child = aux_var_info.as_expr();
            num_changed += 1;
        }
    }

    if num_changed == 0 {
        return Err(RuleNotApplicable);
    }

    let expr = expr.with_children(children);
    Ok(Reduction::new(expr, new_tops, model.variables))
}

#[register_rule(("Minion", 4400))]
fn flatten_vecop(expr: &Expr, model: &Model) -> ApplicationResult {
    if !matches!(expr, Expr::Sum(_, _)) {
        return Err(RuleNotApplicable);
    }

    let mut children = expr.children();

    let mut model = model.clone();
    let mut num_changed = 0;
    let mut new_tops: Vec<Expr> = vec![];

    for child in children.iter_mut() {
        if let Some(aux_var_info) = to_aux_var(child, &model) {
            model = aux_var_info.model();
            new_tops.push(aux_var_info.top_level_expr());
            *child = aux_var_info.as_expr();
            num_changed += 1;
        }
    }

    if num_changed == 0 {
        return Err(RuleNotApplicable);
    }

    let expr = expr.with_children(children);

    Ok(Reduction::new(expr, new_tops, model.variables))
}

#[register_rule(("Minion", 4400))]
fn flatten_eq(expr: &Expr, model: &Model) -> ApplicationResult {
    if !matches!(expr, Expr::Eq(_, _, _)) {
        return Err(RuleNotApplicable);
    }

    let mut children = expr.children();
    debug_assert_eq!(children.len(), 2);

    let mut model = model.clone();
    let mut num_changed = 0;
    let mut new_tops: Vec<Expr> = vec![];

    for child in children.iter_mut() {
        if let Some(aux_var_info) = to_aux_var(child, &model) {
            model = aux_var_info.model();
            new_tops.push(aux_var_info.top_level_expr());
            *child = aux_var_info.as_expr();
            num_changed += 1;
        }
    }

    // eq: both sides have to be non flat for the rule to be applicable!
    if num_changed != 2 {
        return Err(RuleNotApplicable);
    }

    let expr = expr.with_children(children);

    Ok(Reduction::new(expr, new_tops, model.variables))
}

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
            Box::new(Atomic(Metadata::new(), Literal(Int(-1)))),
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
            Box::new(Atomic(Metadata::new(), Literal(Int(-1)))),
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
            Box::new(Atomic(Metadata::new(), Literal(Int(0)))),
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
            Box::new(Atomic(Metadata::new(), Literal(Int(0)))),
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

    let x @ Atomic(_, Reference(_)) = *x.to_owned() else {
        return Err(RuleNotApplicable);
    };

    let Sum(_, c) = *b.to_owned() else {
        return Err(RuleNotApplicable);
    };

    let [ref y @ Atomic(_, Reference(_)), ref k @ Atomic(_, Literal(_))] = c[..] else {
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

/// Flattening rule for not(bool_lit)
///
/// For some boolean variable x:
/// ```text
///  not(x)      ~>  w-literal(x,0)
/// ```
///
/// ## Rationale
///
/// Minion's watched-and and watched-or constraints only takes other constraints as arguments.
///
/// This restates boolean variables as the equivalent constraint "SAT if x is true".
///
/// The regular bool_lit case is dealt with directly by the Minion solver interface (as it is a
/// trivial match).

#[register_rule(("Minion", 4100))]
fn not_literal_to_wliteral(expr: &Expr, mdl: &Model) -> ApplicationResult {
    use Domain::BoolDomain;
    match expr {
        Not(m, expr) => {
            if let Atomic(_, Reference(name)) = (**expr).clone() {
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
    if !matches!(expr, Not(_,c) if !matches!(**c, Atomic(_,_))) {
        return Err(RuleNotApplicable);
    }

    let Not(m, e) = expr else {
        unreachable!();
    };

    extra_check! {
        if !is_flat(e) {
            return Err(RuleNotApplicable);
        }
    };

    Ok(Reduction::pure(Reify(
        m.clone(),
        e.clone(),
        Box::new(Atomic(Metadata::new(), Literal(Bool(false)))),
    )))
}

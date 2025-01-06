/************************************************************************/
/*        Rules for translating to Minion-supported constraints         */
/************************************************************************/

use std::borrow::Borrow as _;

use crate::ast::{Atom, Domain, Expression as Expr, Literal as Lit};

use crate::ast::Name;
use crate::metadata::Metadata;
use crate::rule_engine::{
    register_rule, register_rule_set, ApplicationError, ApplicationResult, Reduction,
};
use crate::rules::extra_check;

use crate::solver::SolverFamily;
use crate::Model;
use uniplate::Uniplate;
use ApplicationError::RuleNotApplicable;

use super::utils::{expressions_to_atoms, is_flat, to_aux_var};

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

            let a_atom: Option<&Atom> = (&a).try_into().ok();
            let b_atom: Option<&Atom> = (&b).try_into().ok();

            if let Some(f) = a_atom {
                // val = div
                val = f.clone();
                div = *b;
            } else if let Some(f) = b_atom {
                // div = val
                val = f.clone();
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
    let a: &Atom = (&children[0]).try_into().or(Err(RuleNotApplicable))?;
    let b: &Atom = (&children[1]).try_into().or(Err(RuleNotApplicable))?;

    Ok(Reduction::pure(Expr::MinionDivEqUndefZero(
        meta.clone_dirty(),
        a.clone(),
        b.clone(),
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
            let a_atom: Option<&Atom> = (&a).try_into().ok();
            let b_atom: Option<&Atom> = (&b).try_into().ok();

            if let Some(f) = a_atom {
                // val = div
                val = f.clone();
                div = *b;
            } else if let Some(f) = b_atom {
                // div = val
                val = f.clone();
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
    let a: &Atom = (&children[0]).try_into().or(Err(RuleNotApplicable))?;
    let b: &Atom = (&children[1]).try_into().or(Err(RuleNotApplicable))?;

    Ok(Reduction::pure(Expr::MinionModuloEqUndefZero(
        meta.clone_dirty(),
        a.clone(),
        b.clone(),
        val,
    )))
}

/// Introduces a Minion `MinusEq` constraint from `x = -y`, where x and y are atoms.
///
/// ```text
/// x = -y ~> MinusEq(x,y)
///
///   where x,y are atoms
/// ```
#[register_rule(("Minion", 4400))]
fn introduce_minuseq_from_eq(expr: &Expr, _: &Model) -> ApplicationResult {
    let Expr::Eq(_, a, b) = expr else {
        return Err(RuleNotApplicable);
    };

    fn try_get_atoms(a: &Expr, b: &Expr) -> Option<(Atom, Atom)> {
        let a: &Atom = a.try_into().ok()?;
        let Expr::Neg(_, b) = b else {
            return None;
        };

        let b: &Atom = b.try_into().ok()?;

        Some((a.clone(), b.clone()))
    }

    let a = *a.clone();
    let b = *b.clone();

    // x = - y. Find this symmetrically (a = - b or b = -a)
    let Some((x, y)) = try_get_atoms(&a, &b).or_else(|| try_get_atoms(&b, &a)) else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::FlatMinusEq(Metadata::new(), x, y)))
}

/// Introduces a Minion `MinusEq` constraint from `x =aux -y`, where x and y are atoms.
///
/// ```text
/// x =aux -y ~> MinusEq(x,y)
///
///   where x,y are atoms
/// ```
#[register_rule(("Minion", 4400))]
fn introduce_minuseq_from_aux_decl(expr: &Expr, _: &Model) -> ApplicationResult {
    // a =aux -b
    //
    let Expr::AuxDeclaration(_, a, b) = expr else {
        return Err(RuleNotApplicable);
    };

    let a = Atom::Reference(a.clone());

    let Expr::Neg(_, b) = (**b).clone() else {
        return Err(RuleNotApplicable);
    };

    let Ok(b) = b.try_into() else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::FlatMinusEq(Metadata::new(), a, b)))
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
    if !matches!(
        expr,
        Expr::Sum(_, _) | Expr::FlatSumGeq(_, _, _) | Expr::FlatSumLeq(_, _, _)
    ) {
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

/// Flattens `a=-e`, where e is a non-atomic expression.
///
/// ```text
/// a = -e ~> a = MinusEq(a,__x), __x =aux e
///  
///  where a is atomic, e is not atomic
/// ```
#[register_rule(("Minion", 4400))]
fn flatten_minuseq(expr: &Expr, m: &Model) -> ApplicationResult {
    // TODO: case where a is a literal not a ref?

    // parses arguments a = -e, where a is an atom and e is a non-atomic expression
    // (when e is an atom, flattening is done, so introduce_minus_eq should be applied instead)
    fn try_get_args(name: &Expr, negated_expr: &Expr) -> Option<(Name, Expr)> {
        let Expr::Atomic(_, Atom::Reference(name)) = name else {
            return None;
        };

        let Expr::Neg(_, e) = negated_expr else {
            return None;
        };

        Some((name.clone(), *e.clone()))
    }

    let (name, e) = match expr {
        // parse arguments symmetrically
        Expr::Eq(_, a, b) => try_get_args(a.borrow(), b.borrow())
            .or_else(|| try_get_args(b.borrow(), a.borrow()))
            .ok_or(RuleNotApplicable),

        Expr::AuxDeclaration(_, name, e) => match e.borrow() {
            Expr::Neg(_, e) => Some((name.clone(), (*e.clone()))),
            _ => None,
        }
        .ok_or(RuleNotApplicable),

        _ => Err(RuleNotApplicable),
    }?;

    let aux_var_out = to_aux_var(&e, m).ok_or(RuleNotApplicable)?;

    let new_expr = Expr::FlatMinusEq(
        Metadata::new(),
        Atom::Reference(name),
        aux_var_out.as_atom(),
    );

    Ok(Reduction::new(
        new_expr,
        vec![aux_var_out.top_level_expr()],
        aux_var_out.model().variables,
    ))
}

// TODO: normalise equalities such that atoms are always on the LHS.
// i.e. always have a = sum(x,y,z), not sum(x,y,z) = a

/// Converts a Geq to a SumGeq if the left hand side is a sum
///
/// ```text
/// sum([a, b, c]) >= d ~> sumgeq([a, b, c], d)
/// ```
#[register_rule(("Minion", 4400))]
fn introduce_sumgeq(expr: &Expr, _: &Model) -> ApplicationResult {
    let Expr::Geq(meta, e1, e2) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let Expr::Sum(_, es) = *e1 else {
        return Err(RuleNotApplicable);
    };

    let Some(atoms) = expressions_to_atoms(&es) else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, rhs) = *e2 else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::FlatSumGeq(meta, atoms, rhs)))
}

/// Converts a Leq to a SumLeq if the left hand side is a sum
///
/// ```text
/// sum([a, b, c]) >= d ~> sumgeq([a, b, c], d)
/// ```
#[register_rule(("Minion", 4400))]
fn introduce_sumleq(expr: &Expr, _: &Model) -> ApplicationResult {
    let Expr::Leq(meta, e1, e2) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let Expr::Sum(_, es) = *e1 else {
        return Err(RuleNotApplicable);
    };

    let Some(atoms) = expressions_to_atoms(&es) else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, rhs) = *e2 else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::FlatSumLeq(meta, atoms, rhs)))
}

/// Converts a 'Eq(Sum([...]))' to a SumEq
/// ```text
/// eq(sum([a, b]), c) => sumeq([a, b], c)
/// ```

#[register_rule(("Minion", 4200))]
fn sum_eq_to_sumeq(expr: &Expr, _: &Model) -> ApplicationResult {
    fn try_get_args(sum_expr: &Expr, value: &Expr) -> Option<(Vec<Expr>, Expr)> {
        let Expr::Sum(_, xs) = sum_expr else {
            return None;
        };

        Some((xs.clone(), value.clone()))
    }

    let (xs, value) = match expr {
        Expr::Eq(_, a, b) => {
            // get arguments symmetrically
            try_get_args(a, b)
                .or_else(|| try_get_args(b, a))
                .ok_or(RuleNotApplicable)
        }

        Expr::AuxDeclaration(_, name, e) => {
            let value = Atom::Reference(name.clone()).into();
            let xs = match *e.clone() {
                Expr::Sum(_, xs) => Ok(xs),
                _ => Err(RuleNotApplicable),
            }?;

            Ok((xs, value))
        }

        _ => Err(RuleNotApplicable),
    }?;

    Ok(Reduction::pure(Expr::SumEq(
        Metadata::new(),
        xs,
        Box::new(value),
    )))
}

/// Converts a `SumEq` to an `And(SumGeq, SumLeq)`
///
/// This is a workaround for Minion not having support for a flat "equals" operation on sums
///
/// ```text
/// sumeq([a, b], c) -> watched_and({
///   sumleq([a, b], c),
///   sumgeq([a, b], c)
/// })
/// ```
#[register_rule(("Minion", 4400))]
fn sumeq_to_minion(expr: &Expr, _: &Model) -> ApplicationResult {
    let Expr::SumEq(_, exprs, eq_to) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let Some(atoms) = expressions_to_atoms(&exprs) else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, eq_to_atom) = *eq_to else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::And(
        Metadata::new(),
        vec![
            Expr::FlatSumLeq(Metadata::new(), atoms.clone(), eq_to_atom.clone()),
            Expr::FlatSumGeq(Metadata::new(), atoms, eq_to_atom),
        ],
    )))
}

/**
* Convert a Lt to an Ineq

* ```text
* x < y ~> x <= y -1 ~> ineq(x,y,-1)
* ```
*/
#[register_rule(("Minion", 4100))]
fn lt_to_ineq(expr: &Expr, _: &Model) -> ApplicationResult {
    let Expr::Lt(meta, e1, e2) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, x) = *e1 else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, y) = *e2 else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::FlatIneq(
        meta.clone_dirty(),
        x,
        y,
        Lit::Int(-1),
    )))
}

/// Converts a Gt to an Ineq
///
/// ```text
/// x > y ~> y <= x -1 ~> ineq(y,x,-1)
/// ```
#[register_rule(("Minion", 4100))]
fn gt_to_ineq(expr: &Expr, _: &Model) -> ApplicationResult {
    let Expr::Gt(meta, e1, e2) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, x) = *e1 else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, y) = *e2 else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::FlatIneq(
        meta.clone_dirty(),
        y,
        x,
        Lit::Int(-1),
    )))
}

/// Converts a Geq to an Ineq
///
/// ```text
/// x >= y ~> y <= x + 0 ~> ineq(y,x,0)
/// ```
#[register_rule(("Minion", 4100))]
fn geq_to_ineq(expr: &Expr, _: &Model) -> ApplicationResult {
    let Expr::Geq(meta, e1, e2) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, x) = *e1 else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, y) = *e2 else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::FlatIneq(
        meta.clone_dirty(),
        y,
        x,
        Lit::Int(0),
    )))
}

/// Converts a Leq to an Ineq
///
/// ```text
/// x <= y ~> x <= y + 0 ~> ineq(x,y,0)
/// ```
#[register_rule(("Minion", 4100))]
fn leq_to_ineq(expr: &Expr, _: &Model) -> ApplicationResult {
    let Expr::Leq(meta, e1, e2) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, x) = *e1 else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, y) = *e2 else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::FlatIneq(
        meta.clone_dirty(),
        x,
        y,
        Lit::Int(0),
    )))
}

// TODO: add this rule for geq

/// ```text
/// x <= y + k ~> ineq(x,y,k)
/// ```
#[register_rule(("Minion",4400))]
fn x_leq_y_plus_k_to_ineq(expr: &Expr, _: &Model) -> ApplicationResult {
    let Expr::Leq(meta, e1, e2) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, x) = *e1 else {
        return Err(RuleNotApplicable);
    };

    let Expr::Sum(_, sum_exprs) = *e2 else {
        return Err(RuleNotApplicable);
    };

    let [Expr::Atomic(_, y), Expr::Atomic(_, Atom::Literal(k))] = sum_exprs.as_slice() else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::FlatIneq(
        meta.clone_dirty(),
        x,
        y.clone(),
        k.clone(),
    )))
}

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
        Expr::Not(m, expr) => {
            if let Expr::Atomic(_, Atom::Reference(name)) = (**expr).clone() {
                if mdl
                    .get_domain(&name)
                    .is_some_and(|x| matches!(x, BoolDomain))
                {
                    return Ok(Reduction::pure(Expr::FlatWatchedLiteral(
                        m.clone_dirty(),
                        name.clone(),
                        Lit::Bool(false),
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
    if !matches!(expr, Expr::Not(_,c) if !matches!(**c, Expr::Atomic(_,_))) {
        return Err(RuleNotApplicable);
    }

    let Expr::Not(m, e) = expr else {
        unreachable!();
    };

    extra_check! {
        if !is_flat(e) {
            return Err(RuleNotApplicable);
        }
    };

    Ok(Reduction::pure(Expr::MinionReify(
        m.clone(),
        e.clone(),
        Atom::Literal(Lit::Bool(false)),
    )))
}

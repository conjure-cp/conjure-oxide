/************************************************************************/
/*        Rules for translating to Minion-supported constraints         */
/************************************************************************/

use std::borrow::Borrow as _;

use crate::ast::{Atom, DecisionVariable, Domain, Expression as Expr, Literal as Lit};

use crate::ast::Name;
use crate::metadata::Metadata;
use crate::rule_engine::{
    register_rule, register_rule_set, ApplicationError, ApplicationResult, Reduction,
};
use crate::rules::extra_check;

use crate::solver::SolverFamily;
use crate::Model;
use uniplate::Uniplate;
use ApplicationError::*;

use super::utils::{expressions_to_atoms, is_flat, to_aux_var};

register_rule_set!("Minion", 100, ("Base"), (SolverFamily::Minion));

#[register_rule(("Minion", 4200))]
fn introduce_producteq(expr: &Expr, model: &Model) -> ApplicationResult {
    // product = val
    let val: Atom;
    let product: Expr;

    match expr.clone() {
        Expr::Eq(_m, a, b) => {
            let a_atom: Option<&Atom> = (&a).try_into().ok();
            let b_atom: Option<&Atom> = (&b).try_into().ok();

            if let Some(f) = a_atom {
                // val = product
                val = f.clone();
                product = *b;
            } else if let Some(f) = b_atom {
                // product = val
                val = f.clone();
                product = *a;
            } else {
                return Err(RuleNotApplicable);
            }
        }
        Expr::AuxDeclaration(_m, name, e) => {
            val = name.into();
            product = *e;
        }
        _ => {
            return Err(RuleNotApplicable);
        }
    }

    if !(matches!(product, Expr::Product(_, _,))) {
        return Err(RuleNotApplicable);
    }

    let Expr::Product(_, mut factors) = product else {
        return Err(RuleNotApplicable);
    };

    if factors.len() < 2 {
        return Err(RuleNotApplicable);
    }

    // Product is a vecop, but FlatProductEq a binop.
    // Introduce auxvars until it is a binop

    // the expression returned will be x*y=val.
    // if factors is > 2 arguments, y will be an auxiliary variable

    #[allow(clippy::unwrap_used)] // should never panic - length is checked above
    let x: Atom = factors
        .pop()
        .unwrap()
        .try_into()
        .or(Err(RuleNotApplicable))?;

    #[allow(clippy::unwrap_used)] // should never panic - length is checked above
    let mut y: Atom = factors
        .pop()
        .unwrap()
        .try_into()
        .or(Err(RuleNotApplicable))?;

    let mut model = model.clone();
    let mut new_tops: Vec<Expr> = vec![];

    // FIXME: add a test for this
    while let Some(next_factor) = factors.pop() {
        // Despite adding auxvars, I still require all atoms as factors, making this rule act
        // similar to other introduction rules.
        let next_factor_atom: Atom = next_factor.clone().try_into().or(Err(RuleNotApplicable))?;

        let aux_var = model.gensym();
        // TODO: find this domain without having to make unnecessary Expr and Metadata objects
        // Just using the domain of expr doesn't work
        let aux_domain = Expr::Product(Metadata::new(), vec![y.clone().into(), next_factor])
            .domain_of(&model.variables)
            .ok_or(ApplicationError::DomainError)?;

        model.add_variable(aux_var.clone(), DecisionVariable { domain: aux_domain });

        let new_top_expr =
            Expr::FlatProductEq(Metadata::new(), y, next_factor_atom, aux_var.clone().into());

        new_tops.push(new_top_expr);
        y = aux_var.into();
    }

    Ok(Reduction::new(
        Expr::FlatProductEq(Metadata::new(), x, y, val),
        new_tops,
        model.variables,
    ))
}

/// Introduces `FlatWeightedSumLeq` / `FlatWeightedSumGeq` constraints.
///
/// # Details
/// This rule is a bit unusual compared to other introduction rules in that
/// it does its own flattening.
///
/// For example, introduce_sumleq only accepts sums of atoms and relies on
/// the generic flattening rules (flatten_binop / flatten_vecop) to run
/// before it flattens the sum.
///
/// ```text
/// a + (b*c) + (d*e) <= 10
///   ~~> flatten_vecop
/// a + __0 + __1 <= 10
///
/// with new top level constraints
///
/// __0 =aux b*c
/// __1 =aux d*e
///
/// ---
///
/// a + __0 + __1 <= 10
///   ~~> introduce_sumleq
/// flat_sumleq([a,__0,__1],10)
///
/// ...
/// ```
///
/// However, weighted sums are expressed as sums of products, which are not
/// flat. Flattening a weighted sum generically makes it indistinguishable
/// from a sum:
///
///```text
/// 1*a + 2*b + 3*c + d <= 10
///   ~~> flatten_vecop
/// __0 + __1 + __2 + d <= 10
///
/// with new top level constraints
///
/// __0 =aux 1*a
/// __1 =aux 2*b
/// __2 =aux 3*c
/// ```
///
/// Therefore, introduce_weightedsumleq_sumgeq does its own flattening, and
/// has a higher priority than flatten_vecop to prevent weighted sums being
/// generically flattened.
///
/// Having custom flattening semantics means that we can make more things
/// weighted sums.
///
/// For example, consider `a + 2*b + 3*c*d + (e / f) + 5*(g/h) <= 18`. This
/// rule turns this into a single flat_weightedsumleq constraint:
///
///```text
/// a + 2*b + 3*c*d + (e/f) + 5*(g/h) <= 30
///
///   ~> introduce_weightedsumleq_sumgeq
///
/// flat_weightedsumleq([1,2,3,1,5],[a,b,__0,__1,__2],30)
///
/// with new top level constraints
///
/// __0 = c*d
/// __1 = e/f
/// __2 = g/h
/// ```
///
/// The rules to turn terms into coefficient variable pairs are the following:
///
/// 1. Non-weighted atom: `a ~> (1,a)`
/// 2. Other non-weighted term: `e ~> (1,__0)`, with new constraint `__0 =aux e`
/// 3. Weighted atom: `c*a ~> (c,a)`
/// 4. Weighted non-atom: `c*e ~> (c,__0)` with new constraint` __0 =aux e`
/// 5. Weighted product: `c*e*f ~> (c,__0)` with new constraint `__0 =aux (e*f)`
#[register_rule(("Minion", 4500))]
fn introduce_weighted_sumleq_sumgeq(expr: &Expr, model: &Model) -> ApplicationResult {
    // assume sum on lhs of leq/geq, as in introduce_sumleq
    // is_leq is true if leq, false if geq
    let (sum_expr, total, is_leq) = match expr.clone() {
        Expr::Leq(_, sum_expr, total) => (*sum_expr, *total, true),
        Expr::Geq(_, sum_expr, total) => (*sum_expr, *total, false),
        _ => {
            return Err(RuleNotApplicable);
        }
    };

    let total: Atom = total.try_into().or(Err(RuleNotApplicable))?;

    let Expr::Sum(_, sum_exprs) = sum_expr.clone() else {
        return Err(RuleNotApplicable);
    };
    let mut new_top_exprs: Vec<Expr> = vec![];
    let mut model = model.clone();

    let mut coefficients: Vec<Lit> = vec![];
    let mut vars: Vec<Atom> = vec![];

    // if all coefficients are 1, use normal sum rule instead
    let mut found_non_one_coeff = false;

    // for each sub-term, get the coefficient and the variable, flattening if necessary.
    for expr in sum_exprs {
        let (coeff, var): (Lit, Atom) = match expr {
            // atom: v ~> 1*v
            Expr::Atomic(_, atom) => (Lit::Int(1), atom),

            // assuming normalisation / partial eval, literal will be first term

            // weighted sum term: c * e.
            // e can either be an atom, the rest of the product to be flattened, or an other expression that needs
            // flattening
            Expr::Product(_, factors)
                if factors.len() > 1 && matches!(factors[0], Expr::Atomic(_, Atom::Literal(_))) =>
            {
                match &factors[..] {
                    // c * <atom>
                    [Expr::Atomic(_, Atom::Literal(c)), Expr::Atomic(_, atom)] => {
                        (c.clone(), atom.clone())
                    }

                    // c * <some non-flat expression>
                    [Expr::Atomic(_, Atom::Literal(c)), e1] => {
                        #[allow(clippy::unwrap_used)] // aux var failing is a bug
                        let aux_var_info = to_aux_var(e1, &model).unwrap();

                        model = aux_var_info.model();
                        let var = aux_var_info.as_atom();
                        new_top_exprs.push(aux_var_info.top_level_expr());
                        (c.clone(), var)
                    }

                    // c * a * b * c * ...
                    [Expr::Atomic(_, Atom::Literal(c)), ref rest @ ..] => {
                        let e1 = Expr::Product(Metadata::new(), rest.to_vec());

                        #[allow(clippy::unwrap_used)] // aux var failing is a bug
                        let aux_var_info = to_aux_var(&e1, &model).unwrap();

                        model = aux_var_info.model();
                        let var = aux_var_info.as_atom();
                        new_top_exprs.push(aux_var_info.top_level_expr());
                        (c.clone(), var)
                    }

                    _ => unreachable!(),
                }
            }

            // flatten non-flat terms without coefficients: e1 ~> (1,__0)
            //
            // includes products without coefficients.
            e => {
                //
                let aux_var_info = to_aux_var(&e, &model).ok_or(RuleNotApplicable)?;

                model = aux_var_info.model();
                let var = aux_var_info.as_atom();
                new_top_exprs.push(aux_var_info.top_level_expr());
                (Lit::Int(1), var)
            }
        };

        let coeff_num: i32 = coeff.clone().try_into().or(Err(RuleNotApplicable))?;
        found_non_one_coeff |= coeff_num != 1;
        coefficients.push(coeff);
        vars.push(var);
    }

    // the expr should use a regular sum instead if the coefficients are all 1.
    if !found_non_one_coeff {
        return Err(RuleNotApplicable);
    }

    let new_expr: Expr = if is_leq {
        Expr::FlatWeightedSumLeq(Metadata::new(), coefficients, vars, total)
    } else {
        Expr::FlatWeightedSumGeq(Metadata::new(), coefficients, vars, total)
    };

    Ok(Reduction::new(new_expr, new_top_exprs, model.variables))
}

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

#[register_rule(("Minion", 4200))]
fn flatten_binop(expr: &Expr, model: &Model) -> ApplicationResult {
    if !matches!(
        expr,
        Expr::SafeDiv(_, _, _)
            | Expr::Neq(_, _, _)
            | Expr::SafeMod(_, _, _)
            | Expr::Leq(_, _, _)
            | Expr::Geq(_, _, _)
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

#[register_rule(("Minion", 4200))]
fn flatten_vecop(expr: &Expr, model: &Model) -> ApplicationResult {
    if !matches!(expr, Expr::Sum(_, _) | Expr::Product(_, _)) {
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

#[register_rule(("Minion", 4200))]
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
#[register_rule(("Minion", 4200))]
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
#[register_rule(("Minion",4500))]
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

    let (y, k) = match sum_exprs.as_slice() {
        [Expr::Atomic(_, y), Expr::Atomic(_, Atom::Literal(k))] => (y, k),
        [Expr::Atomic(_, Atom::Literal(k)), Expr::Atomic(_, y)] => (y, k),
        _ => {
            return Err(RuleNotApplicable);
        }
    };

    Ok(Reduction::pure(Expr::FlatIneq(
        meta.clone_dirty(),
        x,
        y.clone(),
        k.clone(),
    )))
}

/// ```text
/// y + k >= x ~> ineq(x,y,k)
/// ```
#[register_rule(("Minion",4500))]
fn y_plus_k_geq_x_to_ineq(expr: &Expr, _: &Model) -> ApplicationResult {
    // impl same as x_leq_y_plus_k but with lhs and rhs flipped
    let Expr::Geq(meta, e2, e1) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, x) = *e1 else {
        return Err(RuleNotApplicable);
    };

    let Expr::Sum(_, sum_exprs) = *e2 else {
        return Err(RuleNotApplicable);
    };

    let (y, k) = match sum_exprs.as_slice() {
        [Expr::Atomic(_, y), Expr::Atomic(_, Atom::Literal(k))] => (y, k),
        [Expr::Atomic(_, Atom::Literal(k)), Expr::Atomic(_, y)] => (y, k),
        _ => {
            return Err(RuleNotApplicable);
        }
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

// FIXME: updatedisplay impl

//FIXME: refactor symmetry checking for eq into its own function

/// Decomposes `sum(....) = e` into `sum(...) =< e /\`sum(...) >= e`
///
/// # Rationale
/// Minion only has `SumLeq` and `SumGeq` constraints.
#[register_rule(("Minion", 4100))]
fn sum_eq_to_inequalities(expr: &Expr, _: &Model) -> ApplicationResult {
    //
    let (sum, e1): (Box<Expr>, Box<Expr>) = match expr.clone() {
        Expr::Eq(_, e1, e2) if matches!(*e1, Expr::Sum(_, _)) => Ok((e1, e2)),
        Expr::Eq(_, e1, e2) if matches!(*e2, Expr::Sum(_, _)) => Ok((e2, e1)),

        Expr::AuxDeclaration(_, name, e1) if matches!(*e1, Expr::Sum(_, _)) => {
            Ok((e1, Box::new(Expr::Atomic(Metadata::new(), name.into()))))
        }
        _ => Err(RuleNotApplicable),
    }?;

    Ok(Reduction::pure(Expr::And(
        Metadata::new(),
        vec![
            Expr::Leq(Metadata::new(), sum.clone(), e1.clone()),
            Expr::Geq(Metadata::new(), sum.clone(), e1),
        ],
    )))
}

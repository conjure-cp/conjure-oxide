/************************************************************************/
/*        Rules for translating to Minion-supported constraints         */
/************************************************************************/

use std::convert::TryInto;
use std::rc::Rc;

use crate::{
    extra_check,
    utils::{is_flat, to_aux_var},
};
use conjure_core::{
    ast::{
        Atom, Declaration, Domain, Expression as Expr, Literal as Lit, Range, ReturnType,
        SymbolTable,
    },
    matrix_expr,
    metadata::Metadata,
    rule_engine::{
        register_rule, register_rule_set, ApplicationError, ApplicationResult, Reduction,
    },
    solver::SolverFamily,
};

use itertools::Itertools;
use uniplate::Uniplate;

use ApplicationError::*;

register_rule_set!("Minion", ("Base"), (SolverFamily::Minion));

#[register_rule(("Minion", 4200))]
fn introduce_producteq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
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

    let mut symbols = symbols.clone();
    let mut new_tops: Vec<Expr> = vec![];

    // FIXME: add a test for this
    while let Some(next_factor) = factors.pop() {
        // Despite adding auxvars, I still require all atoms as factors, making this rule act
        // similar to other introduction rules.
        let next_factor_atom: Atom = next_factor.clone().try_into().or(Err(RuleNotApplicable))?;

        let aux_var = symbols.gensym();
        // TODO: find this domain without having to make unnecessary Expr and Metadata objects
        // Just using the domain of expr doesn't work
        let aux_domain = Expr::Product(Metadata::new(), vec![y.clone().into(), next_factor])
            .domain_of(&symbols)
            .ok_or(ApplicationError::DomainError)?;

        symbols.insert(Rc::new(Declaration::new_var(aux_var.clone(), aux_domain)));

        let new_top_expr =
            Expr::FlatProductEq(Metadata::new(), y, next_factor_atom, aux_var.clone().into());

        new_tops.push(new_top_expr);
        y = aux_var.into();
    }

    Ok(Reduction::new(
        Expr::FlatProductEq(Metadata::new(), x, y, val),
        new_tops,
        symbols,
    ))
}

/// Introduces `FlatWeightedSumLeq`, `FlatWeightedSumGeq`, `FlatSumLeq`, FlatSumGeq` constraints.
///
/// If the input is a weighted sum, the weighted sum constraints are used, otherwise the standard
/// sum constraints are used.
///
/// # Details
/// This rule is a bit unusual compared to other introduction rules in that
/// it does its own flattening.
///
/// Weighted sums are expressed as sums of products, which are not
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
/// Therefore, introduce_weightedsumleq_sumgeq does its own flattening.
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
/// 6. Negated atom: `-x ~> (-1,x)`
/// 7. Negated expression: `-e ~> (-1,__0)` with new constraint `__0 = e`
///
/// Cases 6 and 7 could potentially be a normalising rule `-e ~> -1*e`. However, I think that we
/// should only turn negations into a product when they are inside a sum, not all the time.
#[register_rule(("Minion", 4600))]
fn introduce_weighted_sumleq_sumgeq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // Keep track of which type of (in)equality was in the input, and use this to decide what
    // constraints to make at the end

    // We handle Eq directly in this rule instead of letting it be decomposed to <= and >=
    // elsewhere, as this caused cyclic rule application:
    //
    // ```
    // 2*a + b = c
    //
    //   ~~> sumeq_to_inequalities
    //
    // 2*a + b <=c /\ 2*a + b >= c
    //
    // --
    //
    // 2*a + b <= c
    //
    //   ~~> flatten_generic
    // __1 <=c
    //
    // with new top level constraint
    //
    // 2*a + b =aux __1
    //
    // --
    //
    // 2*a + b =aux __1
    //
    //   ~~> sumeq_to_inequalities
    //
    // LOOP!
    // ```
    enum EqualityKind {
        Eq,
        Leq,
        Geq,
    }

    // Given the LHS, RHS, and the type of inequality, return the sum, total, and new inequality.
    //
    // The inequality returned is the one that puts the sum is on the left hand side and the total
    // on the right hand side.
    //
    // For example, `1 <= a + b` will result in ([a,b],1,Geq).
    fn match_sum_total(
        a: Expr,
        b: Expr,
        equality_kind: EqualityKind,
    ) -> Result<(Vec<Expr>, Atom, EqualityKind), ApplicationError> {
        match (a, b, equality_kind) {
            (Expr::Sum(_, sum_terms), Expr::Atomic(_, total), EqualityKind::Leq) => {
                let sum_terms = sum_terms.unwrap_list().ok_or(RuleNotApplicable)?;
                Ok((sum_terms, total, EqualityKind::Leq))
            }
            (Expr::Atomic(_, total), Expr::Sum(_, sum_terms), EqualityKind::Leq) => {
                let sum_terms = sum_terms.unwrap_list().ok_or(RuleNotApplicable)?;
                Ok((sum_terms, total, EqualityKind::Geq))
            }
            (Expr::Sum(_, sum_terms), Expr::Atomic(_, total), EqualityKind::Geq) => {
                let sum_terms = sum_terms.unwrap_list().ok_or(RuleNotApplicable)?;
                Ok((sum_terms, total, EqualityKind::Geq))
            }
            (Expr::Atomic(_, total), Expr::Sum(_, sum_terms), EqualityKind::Geq) => {
                let sum_terms = sum_terms.unwrap_list().ok_or(RuleNotApplicable)?;
                Ok((sum_terms, total, EqualityKind::Leq))
            }
            (Expr::Sum(_, sum_terms), Expr::Atomic(_, total), EqualityKind::Eq) => {
                let sum_terms = sum_terms.unwrap_list().ok_or(RuleNotApplicable)?;
                Ok((sum_terms, total, EqualityKind::Eq))
            }
            (Expr::Atomic(_, total), Expr::Sum(_, sum_terms), EqualityKind::Eq) => {
                let sum_terms = sum_terms.unwrap_list().ok_or(RuleNotApplicable)?;
                Ok((sum_terms, total, EqualityKind::Eq))
            }
            _ => Err(RuleNotApplicable),
        }
    }

    let (sum_exprs, total, equality_kind) = match expr.clone() {
        Expr::Leq(_, a, b) => Ok(match_sum_total(*a, *b, EqualityKind::Leq)?),
        Expr::Geq(_, a, b) => Ok(match_sum_total(*a, *b, EqualityKind::Geq)?),
        Expr::Eq(_, a, b) => Ok(match_sum_total(*a, *b, EqualityKind::Eq)?),
        Expr::AuxDeclaration(_, n, a) => {
            let total: Atom = n.into();
            if let Expr::Sum(_, sum_terms) = *a {
                let sum_terms = sum_terms.unwrap_list().ok_or(RuleNotApplicable)?;
                Ok((sum_terms, total, EqualityKind::Eq))
            } else {
                Err(RuleNotApplicable)
            }
        }
        _ => Err(RuleNotApplicable),
    }?;

    let mut new_top_exprs: Vec<Expr> = vec![];
    let mut symbols = symbols.clone();

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
                        let aux_var_info = to_aux_var(e1, &symbols).unwrap();

                        symbols = aux_var_info.symbols();
                        let var = aux_var_info.as_atom();
                        new_top_exprs.push(aux_var_info.top_level_expr());
                        (c.clone(), var)
                    }

                    // c * a * b * c * ...
                    [Expr::Atomic(_, Atom::Literal(c)), ref rest @ ..] => {
                        let e1 = Expr::Product(Metadata::new(), rest.to_vec());

                        #[allow(clippy::unwrap_used)] // aux var failing is a bug
                        let aux_var_info = to_aux_var(&e1, &symbols).unwrap();

                        symbols = aux_var_info.symbols();
                        let var = aux_var_info.as_atom();
                        new_top_exprs.push(aux_var_info.top_level_expr());
                        (c.clone(), var)
                    }

                    _ => unreachable!(),
                }
            }

            // negated terms: `-e ~> -1*e`
            //
            // flatten e if non-atomic
            Expr::Neg(_, e) => {
                // needs flattening
                let v: Atom = if let Some(aux_var_info) = to_aux_var(&e, &symbols) {
                    symbols = aux_var_info.symbols();
                    new_top_exprs.push(aux_var_info.top_level_expr());
                    aux_var_info.as_atom()
                } else {
                    // if we can't flatten it, it must be an atom!
                    #[allow(clippy::unwrap_used)]
                    e.try_into().unwrap()
                };

                (Lit::Int(-1), v)
            }

            // flatten non-flat terms without coefficients: e1 ~> (1,__0)
            //
            // includes products without coefficients.
            e => {
                //
                let aux_var_info = to_aux_var(&e, &symbols).ok_or(RuleNotApplicable)?;

                symbols = aux_var_info.symbols();
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

    let use_weighted_sum = found_non_one_coeff;
    // the expr should use a regular sum instead if the coefficients are all 1.
    let new_expr: Expr = match (equality_kind, use_weighted_sum) {
        (EqualityKind::Eq, true) => Expr::And(
            Metadata::new(),
            Box::new(matrix_expr![
                Expr::FlatWeightedSumLeq(
                    Metadata::new(),
                    coefficients.clone(),
                    vars.clone(),
                    total.clone(),
                ),
                Expr::FlatWeightedSumGeq(Metadata::new(), coefficients, vars, total),
            ]),
        ),
        (EqualityKind::Eq, false) => Expr::And(
            Metadata::new(),
            Box::new(matrix_expr![
                Expr::FlatSumLeq(Metadata::new(), vars.clone(), total.clone()),
                Expr::FlatSumGeq(Metadata::new(), vars, total),
            ]),
        ),
        (EqualityKind::Leq, true) => {
            Expr::FlatWeightedSumLeq(Metadata::new(), coefficients, vars, total)
        }
        (EqualityKind::Leq, false) => Expr::FlatSumLeq(Metadata::new(), vars, total),
        (EqualityKind::Geq, true) => {
            Expr::FlatWeightedSumGeq(Metadata::new(), coefficients, vars, total)
        }
        (EqualityKind::Geq, false) => Expr::FlatSumGeq(Metadata::new(), vars, total),
    };

    Ok(Reduction::new(new_expr, new_top_exprs, symbols))
}

#[register_rule(("Minion", 4200))]
fn introduce_diveq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
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
fn introduce_modeq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
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

#[register_rule(("Minion", 4400))]
fn introduce_abseq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (x, abs_y): (Atom, Expr) = match expr.clone() {
        Expr::Eq(_, a, b) => {
            let a_atom: Option<&Atom> = (&*a).try_into().ok();
            let b_atom: Option<&Atom> = (&*b).try_into().ok();

            if let Some(a_atom) = a_atom {
                Ok((a_atom.clone(), *b))
            } else if let Some(b_atom) = b_atom {
                Ok((b_atom.clone(), *a))
            } else {
                Err(RuleNotApplicable)
            }
        }

        Expr::AuxDeclaration(_, a, b) => Ok((a.into(), *b)),

        _ => Err(RuleNotApplicable),
    }?;

    let Expr::Abs(_, y) = abs_y else {
        return Err(RuleNotApplicable);
    };

    let y: Atom = (*y).try_into().or(Err(RuleNotApplicable))?;

    Ok(Reduction::pure(Expr::FlatAbsEq(Metadata::new(), x, y)))
}

/// Introduces a `MinionPowEq` constraint from a `SafePow`
#[register_rule(("Minion", 4200))]
fn introduce_poweq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (a, b, total) = match expr.clone() {
        Expr::Eq(_, e1, e2) => match (*e1, *e2) {
            (Expr::Atomic(_, total), Expr::SafePow(_, a, b)) => Ok((a, b, total)),
            (Expr::SafePow(_, a, b), Expr::Atomic(_, total)) => Ok((a, b, total)),
            _ => Err(RuleNotApplicable),
        },

        Expr::AuxDeclaration(_, total, e) => match *e {
            Expr::SafePow(_, a, b) => Ok((a, b, Atom::Reference(total))),
            _ => Err(RuleNotApplicable),
        },
        _ => Err(RuleNotApplicable),
    }?;

    let a: Atom = (*a).try_into().or(Err(RuleNotApplicable))?;
    let b: Atom = (*b).try_into().or(Err(RuleNotApplicable))?;

    Ok(Reduction::pure(Expr::MinionPow(
        Metadata::new(),
        a,
        b,
        total,
    )))
}

/// Introduces a `FlatAlldiff` constraint from an `AllDiff`
#[register_rule(("Minion", 4200))]
fn introduce_flat_alldiff(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::AllDiff(_, es) = expr else {
        return Err(RuleNotApplicable);
    };

    let es = es.clone().unwrap_list().ok_or(RuleNotApplicable)?;

    let atoms = es
        .into_iter()
        .map(|e| match e {
            Expr::Atomic(_, atom) => Ok(atom),
            _ => Err(RuleNotApplicable),
        })
        .process_results(|iter| iter.collect_vec())?;

    Ok(Reduction::pure(Expr::FlatAllDiff(Metadata::new(), atoms)))
}

/// Introduces a Minion `MinusEq` constraint from `x = -y`, where x and y are atoms.
///
/// ```text
/// x = -y ~> MinusEq(x,y)
///
///   where x,y are atoms
/// ```
#[register_rule(("Minion", 4400))]
fn introduce_minuseq_from_eq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
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
fn introduce_minuseq_from_aux_decl(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
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

/// Converts an implication to either `ineq` or `reifyimply`
///
/// ```text
/// x -> y ~> ineq(x,y,0)
/// where x is atomic, y is atomic
///
/// x -> y ~> reifyimply(y,x)
/// where x is atomic, y is non-atomic
/// ```
#[register_rule(("Minion", 4400))]
fn introduce_reifyimply_ineq_from_imply(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Imply(_, x, y) = expr else {
        return Err(RuleNotApplicable);
    };

    let x_atom: &Atom = x.as_ref().try_into().or(Err(RuleNotApplicable))?;

    // if both x and y are atoms,  x -> y ~> ineq(x,y,0)
    //
    // if only x is an atom, x -> y ~> reifyimply(y,x)
    if let Ok(y_atom) = TryInto::<&Atom>::try_into(y.as_ref()) {
        Ok(Reduction::pure(Expr::FlatIneq(
            Metadata::new(),
            x_atom.clone(),
            y_atom.clone(),
            0.into(),
        )))
    } else {
        Ok(Reduction::pure(Expr::MinionReifyImply(
            Metadata::new(),
            y.clone(),
            x_atom.clone(),
        )))
    }
}

/// Converts `__inDomain(a,domain) to w-inintervalset.
///
/// This applies if domain is integer and finite.
#[register_rule(("Minion", 4400))]
fn introduce_wininterval_set_from_indomain(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::InDomain(_, e, domain) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, atom @ Atom::Reference(_)) = e.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let Domain::IntDomain(ranges) = domain else {
        return Err(RuleNotApplicable);
    };

    let mut out_ranges = vec![];

    for range in ranges {
        match range {
            Range::Single(x) => {
                out_ranges.push(*x);
                out_ranges.push(*x);
            }
            Range::Bounded(x, y) => {
                out_ranges.push(*x);
                out_ranges.push(*y);
            }
            Range::UnboundedR(_) | Range::UnboundedL(_) => {
                return Err(RuleNotApplicable);
            }
        }
    }

    Ok(Reduction::pure(Expr::MinionWInIntervalSet(
        Metadata::new(),
        atom.clone(),
        out_ranges,
    )))
}

/// Converts `[....][i]` to `element_one` if:
///
/// 1. the subject is a list literal
/// 2. the subject is one dimensional
#[register_rule(("Minion", 4400))]
fn introduce_element_from_index(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (equalto, subject, indices) = match expr.clone() {
        Expr::Eq(_, e1, e2) => match (*e1, *e2) {
            (Expr::Atomic(_, eq), Expr::SafeIndex(_, subject, indices)) => {
                Ok((eq, subject, indices))
            }
            (Expr::SafeIndex(_, subject, indices), Expr::Atomic(_, eq)) => {
                Ok((eq, subject, indices))
            }
            _ => Err(RuleNotApplicable),
        },
        Expr::AuxDeclaration(_, name, expr) => match *expr {
            Expr::SafeIndex(_, subject, indices) => Ok((Atom::Reference(name), subject, indices)),
            _ => Err(RuleNotApplicable),
        },
        _ => Err(RuleNotApplicable),
    }?;

    if indices.len() != 1 {
        return Err(RuleNotApplicable);
    }

    let Some(list) = subject.unwrap_list() else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, index) = indices[0].clone() else {
        return Err(RuleNotApplicable);
    };

    let mut atom_list = vec![];

    for elem in list {
        let Expr::Atomic(_, elem) = elem else {
            return Err(RuleNotApplicable);
        };

        atom_list.push(elem);
    }

    Ok(Reduction::pure(Expr::MinionElementOne(
        Metadata::new(),
        atom_list,
        index,
        equalto,
    )))
}

/// Flattens an implication.
///
/// ```text
/// e -> y  (where e is non atomic)
///  ~~>
/// __0 -> y,
///
/// with new top level constraints
/// __0 =aux x
///
/// ```
///
/// Unlike other expressions, only the left hand side of implications are flattened. This is
/// because implications can be expressed as a `reifyimply` constraint, which takes a constraint as
/// an argument:
///
/// ``` text
/// r -> c ~> refifyimply(r,c)
///  where r is an atom, c is a constraint
/// ```
///
/// See [`introduce_reifyimply_ineq_from_imply`].
#[register_rule(("Minion", 4200))]
fn flatten_imply(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Imply(meta, x, y) = expr else {
        return Err(RuleNotApplicable);
    };

    // flatten x
    let aux_var_info = to_aux_var(x.as_ref(), symbols).ok_or(RuleNotApplicable)?;

    let symbols = aux_var_info.symbols();
    let new_x = aux_var_info.as_expr();

    Ok(Reduction::new(
        Expr::Imply(meta.clone(), Box::new(new_x), y.clone()),
        vec![aux_var_info.top_level_expr()],
        symbols,
    ))
}

#[register_rule(("Minion", 4200))]
fn flatten_generic(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    if !matches!(
        expr,
        Expr::SafeDiv(_, _, _)
            | Expr::Neq(_, _, _)
            | Expr::SafeMod(_, _, _)
            | Expr::SafePow(_, _, _)
            | Expr::Leq(_, _, _)
            | Expr::Geq(_, _, _)
            | Expr::Abs(_, _)
            | Expr::Product(_, _)
            | Expr::Neg(_, _)
            | Expr::Not(_, _)
            | Expr::SafeIndex(_, _, _)
            | Expr::InDomain(_, _, _)
    ) {
        return Err(RuleNotApplicable);
    }

    let mut children = expr.children();

    let mut symbols = symbols.clone();
    let mut num_changed = 0;
    let mut new_tops: Vec<Expr> = vec![];

    for child in children.iter_mut() {
        if let Some(aux_var_info) = to_aux_var(child, &symbols) {
            symbols = aux_var_info.symbols();
            new_tops.push(aux_var_info.top_level_expr());
            *child = aux_var_info.as_expr();
            num_changed += 1;
        }
    }

    if num_changed == 0 {
        return Err(RuleNotApplicable);
    }

    let expr = expr.with_children(children);

    Ok(Reduction::new(expr, new_tops, symbols))
}

#[register_rule(("Minion", 4200))]
fn flatten_eq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    if !matches!(expr, Expr::Eq(_, _, _)) {
        return Err(RuleNotApplicable);
    }

    let mut children = expr.children();
    debug_assert_eq!(children.len(), 2);

    let mut symbols = symbols.clone();
    let mut num_changed = 0;
    let mut new_tops: Vec<Expr> = vec![];

    for child in children.iter_mut() {
        if let Some(aux_var_info) = to_aux_var(child, &symbols) {
            symbols = aux_var_info.symbols();
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

    Ok(Reduction::new(expr, new_tops, symbols))
}

/// Converts a Geq to an Ineq
///
/// ```text
/// x >= y ~> y <= x + 0 ~> ineq(y,x,0)
/// ```
#[register_rule(("Minion", 4100))]
fn geq_to_ineq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
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
fn leq_to_ineq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
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
fn x_leq_y_plus_k_to_ineq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Leq(meta, e1, e2) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, x) = *e1 else {
        return Err(RuleNotApplicable);
    };

    let Expr::Sum(_, sum_exprs) = *e2 else {
        return Err(RuleNotApplicable);
    };

    let sum_exprs = sum_exprs.unwrap_list().ok_or(RuleNotApplicable)?;
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
#[register_rule(("Minion",4800))]
fn y_plus_k_geq_x_to_ineq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
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

    let sum_exprs = sum_exprs.unwrap_list().ok_or(RuleNotApplicable)?;
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
fn not_literal_to_wliteral(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    use Domain::BoolDomain;
    match expr {
        Expr::Not(m, expr) => {
            if let Expr::Atomic(_, Atom::Reference(name)) = (**expr).clone() {
                if symbols
                    .domain(&name)
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
fn not_constraint_to_reify(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
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

/// Converts an equality to a boolean into a `reify` constraint.
///
/// ```text
/// x =aux c ~> reify(c,x)
/// x = c ~> reify(c,x)
///
/// where c is a boolean constraint
/// ```
#[register_rule(("Minion", 4400))]
fn bool_eq_to_reify(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (atom, e): (Atom, Box<Expr>) = match expr {
        Expr::AuxDeclaration(_, name, e) => Ok((name.clone().into(), e.clone())),
        Expr::Eq(_, a, b) => match (a.as_ref(), b.as_ref()) {
            (Expr::Atomic(_, atom), _) => Ok((atom.clone(), b.clone())),
            (_, Expr::Atomic(_, atom)) => Ok((atom.clone(), a.clone())),
            _ => Err(RuleNotApplicable),
        },

        _ => Err(RuleNotApplicable),
    }?;

    // e does not have to be valid minion constraint yet, as long as we know it can turn into one
    // (i.e. it is boolean).
    let Some(ReturnType::Bool) = e.as_ref().return_type() else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::MinionReify(
        Metadata::new(),
        e.clone(),
        atom,
    )))
}

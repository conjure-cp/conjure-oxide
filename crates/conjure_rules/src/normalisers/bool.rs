//! Normalising rules for boolean operations (not, and, or, ->).

use conjure_core::ast::{Atom, Expression as Expr, SymbolTable};
use conjure_core::into_matrix_expr;
use conjure_core::rule_engine::{
    register_rule, ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult,
    Reduction,
};
use conjure_essence_macros::essence_expr;
use uniplate::Uniplate;

/// Removes double negations
///
/// ```text
/// not(not(a)) = a
/// ```
#[register_rule(("Base", 8400))]
fn remove_double_negation(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Not(_, contents) => match contents.as_ref() {
            Expr::Not(_, expr_box) => Ok(Reduction::pure(*expr_box.clone())),
            _ => Err(ApplicationError::RuleNotApplicable),
        },
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/// Distributes `ands` contained in `ors`
///
/// ```text
/// or(and(a, b), c) ~> and(or(a, c), or(b, c))
/// ```
#[register_rule(("Base", 8400))]
fn distribute_or_over_and(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    fn find_and(exprs: &[Expr]) -> Option<usize> {
        // ToDo: may be better to move this to some kind of utils module?
        for (i, e) in exprs.iter().enumerate() {
            if let Expr::And(_, _) = e {
                return Some(i);
            }
        }
        None
    }

    match expr {
        Expr::Or(_, e) => {
            let Some(exprs) = e.as_ref().clone().unwrap_list() else {
                return Err(RuleNotApplicable);
            };

            match find_and(&exprs) {
                Some(idx) => {
                    let mut rest = exprs.clone();
                    let and_expr = rest.remove(idx);

                    match and_expr {
                        Expr::And(metadata, e) => {
                            let Some(and_exprs) = e.as_ref().clone().unwrap_list() else {
                                return Err(RuleNotApplicable);
                            };

                            let mut new_and_contents = Vec::new();

                            for e in and_exprs {
                                // ToDo: Cloning everything may be a bit inefficient - discuss
                                let mut new_or_contents = rest.clone();
                                new_or_contents.push(e.clone());
                                new_and_contents.push(Expr::Or(
                                    metadata.clone_dirty(),
                                    Box::new(into_matrix_expr![new_or_contents]),
                                ))
                            }

                            Ok(Reduction::pure(Expr::And(
                                metadata.clone_dirty(),
                                Box::new(into_matrix_expr![new_and_contents]),
                            )))
                        }
                        _ => Err(ApplicationError::RuleNotApplicable),
                    }
                }
                None => Err(ApplicationError::RuleNotApplicable),
            }
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/// Distributes `not` over `and` by De Morgan's Law
///
/// ```text
/// not(and(a, b)) ~> or(not(a), not(b))
/// ```
#[register_rule(("Base", 8400))]
fn distribute_not_over_and(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    for child in expr.universe() {
        if matches!(
            child,
            Expr::UnsafeDiv(_, _, _) | Expr::Bubble(_, _, _) | Expr::UnsafeMod(_, _, _)
        ) {
            return Err(RuleNotApplicable);
        }
    }
    match expr {
        Expr::Not(_, contents) => match contents.as_ref() {
            Expr::And(metadata, e) => {
                let Some(exprs) = e.as_ref().clone().unwrap_list() else {
                    return Err(RuleNotApplicable);
                };

                if exprs.len() == 1 {
                    let single_expr = exprs[0].clone();
                    return Ok(Reduction::pure(essence_expr!(!&single_expr)));
                }

                let mut new_exprs = Vec::new();
                for e in exprs {
                    new_exprs.push(essence_expr!(!&e));
                }
                Ok(Reduction::pure(Expr::Or(
                    metadata.clone(),
                    Box::new(into_matrix_expr![new_exprs]),
                )))
            }
            _ => Err(ApplicationError::RuleNotApplicable),
        },
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/// Distributes `not` over `or` by De Morgan's Law
///
/// ```text
/// not(or(a, b)) ~> and(not(a), not(b))
/// ```
#[register_rule(("Base", 8400))]
fn distribute_not_over_or(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Not(_, contents) => match contents.as_ref() {
            Expr::Or(metadata, e) => {
                let Some(exprs) = e.as_ref().clone().unwrap_list() else {
                    return Err(RuleNotApplicable);
                };

                if exprs.len() == 1 {
                    let single_expr = exprs[0].clone();
                    return Ok(Reduction::pure(essence_expr!(!&single_expr)));
                }

                let mut new_exprs = Vec::new();

                for e in exprs {
                    new_exprs.push(essence_expr!(!&e));
                }

                Ok(Reduction::pure(Expr::And(
                    metadata.clone(),
                    Box::new(into_matrix_expr![new_exprs]),
                )))
            }
            _ => Err(ApplicationError::RuleNotApplicable),
        },
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/// Removes ands with a single argument.
///
/// ```text
/// and([a]) ~> a
/// ```
#[register_rule(("Base", 8800))]
fn remove_unit_vector_and(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::And(_, e) => {
            let Some(exprs) = e.as_ref().clone().unwrap_list() else {
                return Err(RuleNotApplicable);
            };

            if exprs.len() == 1 {
                return Ok(Reduction::pure(exprs[0].clone()));
            }

            Err(ApplicationError::RuleNotApplicable)
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/// Removes ors with a single argument.
///
/// ```text
/// or([a]) ~> a
/// ```
#[register_rule(("Base", 8800))]
fn remove_unit_vector_or(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Or(_, e) = expr else {
        return Err(RuleNotApplicable);
    };

    let Some(exprs) = e.as_ref().clone().unwrap_list() else {
        return Err(RuleNotApplicable);
    };

    // do not conflict with unwrap_nested_or rule.
    if exprs.len() != 1 || matches!(exprs[0], Expr::Or(_, _)) {
        return Err(RuleNotApplicable);
    }

    Ok(Reduction::pure(exprs[0].clone()))
}

/// Applies the contrapositive of implication.
///
/// ```text
/// !p -> !q ~> q -> p
/// ```
/// where p,q are safe.
#[register_rule(("Base", 8800))]
fn normalise_implies_contrapositive(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Imply(_, e1, e2) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Not(_, p) = e1.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let Expr::Not(_, q) = e2.as_ref() else {
        return Err(RuleNotApplicable);
    };

    // we only negate e1, e2 if they are safe.
    if !e1.is_safe() || !e2.is_safe() {
        return Err(RuleNotApplicable);
    }

    Ok(Reduction::pure(essence_expr!(&q -> &p)))
}

/// Simplifies the negation of implication.
///
/// ```text
/// !(p->q) ~> p /\ !q
/// ```,
///
/// where p->q is safe
#[register_rule(("Base", 8800))]
fn normalise_implies_negation(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Not(_, e1) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Imply(_, p, q) = e1.as_ref() else {
        return Err(RuleNotApplicable);
    };

    // p->q must be safe to negate
    if !e1.is_safe() {
        return Err(RuleNotApplicable);
    }

    Ok(Reduction::pure(essence_expr!(r"&p /\ !&q")))
}

/// Applies left distributivity to implication.
///
/// ```text
/// ((r -> p) -> (r->q)) ~> (r -> (p -> q))
/// ```
///
/// This rule relies on CSE to unify the two instances of `r` to a single atom; therefore, it might
/// not work as well when optimisations are disabled.
///
/// Has a higher priority than `normalise_implies_uncurry` as this should apply first. See the
/// docstring for `normalise_implies_uncurry`.
#[register_rule(("Base", 8800))]
fn normalise_implies_left_distributivity(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Imply(_, e1, e2) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Imply(_, r1, p) = e1.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let Expr::Imply(_, r2, q) = e2.as_ref() else {
        return Err(RuleNotApplicable);
    };

    // Instead of checking deep equality, let CSE unify them to a common variable and check for
    // that.

    let r1_atom: &Atom = r1.as_ref().try_into().or(Err(RuleNotApplicable))?;
    let r2_atom: &Atom = r2.as_ref().try_into().or(Err(RuleNotApplicable))?;

    if !(r1_atom == r2_atom) {
        return Err(RuleNotApplicable);
    }

    Ok(Reduction::pure(essence_expr!(&r1 -> (&p -> &q))))
}

/// Applies import-export to implication, i.e. uncurrying.
///
/// ```text
/// p -> (q -> r) ~> (p/\q) -> r
/// ```
///
/// This rule has a lower priority of 8400 to allow distributivity, contraposition, etc. to
/// apply first.
///
/// For example, we want to do:
///
/// ```text
/// ((r -> p) -> (r -> q)) ~> (r -> (p -> q))  [left-distributivity]
/// (r -> (p -> q)) ~> (r/\p) ~> q [uncurry]
/// ```
///
/// not
///
/// ```text
/// ((r->p) -> (r->q)) ~> ((r->p) /\ r) -> q) ~> [uncurry]
/// ```
///
/// # Rationale
///
/// With this rule, I am assuming (without empirical evidence) that and is a cheaper constraint
/// than implication (in particular, Minion's reifyimply constraint).
#[register_rule(("Base", 8400))]
fn normalise_implies_uncurry(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Imply(_, p, e1) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Imply(_, q, r) = e1.as_ref() else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(essence_expr!(r"(&p /\ &q) -> &r")))
}

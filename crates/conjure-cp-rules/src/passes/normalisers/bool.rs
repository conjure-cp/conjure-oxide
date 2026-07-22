//! Normalising rules for boolean operations (not, and, or, ->).

use conjure_cp::ast::{Atom, Expression as Expr, Moo, SymbolTable};
use conjure_cp::essence_expr;
use conjure_cp::into_matrix_expr;
use conjure_cp::rule_engine::{
    ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect,
    register_rule,
};
use uniplate::Uniplate;

/// Removes double negations
///
/// ```text
/// not(not(a)) = a
/// ```
#[register_rule("Base", 8400, [Not])]
fn remove_double_negation(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Not(_, contents) => match contents.as_ref() {
            Expr::Not(_, expr_box) => Ok(RuleEffect::pure(Moo::unwrap_or_clone(expr_box.clone()))),
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
///
/// Size-increasing cases with a non-trivial rest are refused: see
/// [`distribution_would_duplicate_nontrivial_rest`]. Nested `and`/`or` remain
/// valid for Minion (`WatchedAnd`/`WatchedOr`) and for SAT Tseytin encoding.
#[register_rule("Base", 8400, [Or])]
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

                            // Refuse CNF blow-ups such as or([and(a,b), and(c,d), ...]) that
                            // copy every sibling disjunct across each conjunct.
                            if distribution_would_duplicate_nontrivial_rest(and_exprs.len(), &rest)
                            {
                                return Err(RuleNotApplicable);
                            }

                            let mut new_and_contents = Vec::new();

                            for e in and_exprs {
                                // ToDo: Cloning everything may be a bit inefficient - discuss
                                let mut new_or_contents = rest.clone();
                                new_or_contents.push(e.clone());
                                new_and_contents.push(Expr::Or(
                                    metadata.clone(),
                                    Moo::new(into_matrix_expr![new_or_contents]),
                                ))
                            }

                            Ok(RuleEffect::pure(Expr::And(
                                metadata,
                                Moo::new(into_matrix_expr![new_and_contents]),
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

/// Returns true when distributing `or(and(a1..ak), r1..rm)` would copy a
/// non-trivial rest across multiple conjuncts.
///
/// Refused when either:
/// - there are two or more rest siblings (`k * m` growth), or
/// - any rest sibling is itself an `And` (DNF seed: `or(and, and)`).
///
/// A single non-`And` rest sibling (`or(and(a,b), c)`) remains allowed.
fn distribution_would_duplicate_nontrivial_rest(and_len: usize, rest: &[Expr]) -> bool {
    and_len >= 2 && (rest.len() >= 2 || rest.iter().any(|e| matches!(e, Expr::And(_, _))))
}

/// Distributes `not` over `and` by De Morgan's Law
///
/// ```text
/// not(and(a, b)) ~> or(not(a), not(b))
/// ```
#[register_rule("Base", 8400, [Not])]
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
                    return Ok(RuleEffect::pure(essence_expr!(!&single_expr)));
                }

                let mut new_exprs = Vec::new();
                for e in exprs {
                    new_exprs.push(essence_expr!(!&e));
                }
                Ok(RuleEffect::pure(Expr::Or(
                    metadata.clone(),
                    Moo::new(into_matrix_expr![new_exprs]),
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
#[register_rule("Base", 8400, [Not])]
fn distribute_not_over_or(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Not(_, contents) => match contents.as_ref() {
            Expr::Or(metadata, e) => {
                let Some(exprs) = e.as_ref().clone().unwrap_list() else {
                    return Err(RuleNotApplicable);
                };

                if exprs.len() == 1 {
                    let single_expr = exprs[0].clone();
                    return Ok(RuleEffect::pure(essence_expr!(!&single_expr)));
                }

                let mut new_exprs = Vec::new();

                for e in exprs {
                    new_exprs.push(essence_expr!(!&e));
                }

                Ok(RuleEffect::pure(Expr::And(
                    metadata.clone(),
                    Moo::new(into_matrix_expr![new_exprs]),
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
#[register_rule("Base", 8800, [And])]

fn remove_unit_vector_and(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::And(_, e) => {
            let Some(exprs) = e.as_ref().clone().unwrap_list() else {
                return Err(RuleNotApplicable);
            };

            if exprs.len() == 1 {
                return Ok(RuleEffect::pure(exprs[0].clone()));
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
#[register_rule("Base", 8800, [Or])]
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

    Ok(RuleEffect::pure(exprs[0].clone()))
}

/// Applies the contrapositive of implication.
///
/// ```text
/// !p -> !q ~> q -> p
/// ```
/// where p,q are safe.
#[register_rule("Base", 8800, [Imply])]
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

    Ok(RuleEffect::pure(essence_expr!(&q -> &p)))
}

/// Simplifies the negation of implication.
///
/// ```text
/// !(p->q) ~> p /\ !q
/// ```,
///
/// where p->q is safe
#[register_rule("Base", 8800, [Not])]
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

    Ok(RuleEffect::pure(essence_expr!(r"&p /\ !&q")))
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
#[register_rule("Base", 8800, [Imply])]
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

    Ok(RuleEffect::pure(essence_expr!(&r1 -> (&p -> &q))))
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
#[register_rule("Base", 8400, [Imply])]
fn normalise_implies_uncurry(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Imply(_, p, e1) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Imply(_, q, r) = e1.as_ref() else {
        return Err(RuleNotApplicable);
    };

    Ok(RuleEffect::pure(essence_expr!(r"(&p /\ &q) -> &r")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Atom, DeclarationPtr, Domain, Metadata, Name, Reference};
    use conjure_cp::matrix_expr;
    use conjure_cp::rule_engine::{ApplicationError, get_rule_by_name};

    /// Builds a boolean decision-variable reference for distribution tests.
    fn bool_ref(machine_id: i32) -> Atom {
        Atom::Reference(Reference::new(DeclarationPtr::new_find(
            Name::Machine(machine_id),
            Domain::bool(),
        )))
    }

    /// Atomic expression wrapping a boolean reference.
    fn atom_expr(atom: Atom) -> Expr {
        Expr::Atomic(Metadata::new(), atom)
    }

    #[test]
    fn distribution_guard_allows_single_atomic_rest_sibling() {
        let rest = [atom_expr(bool_ref(9))];
        assert!(!distribution_would_duplicate_nontrivial_rest(2, &rest));
    }

    #[test]
    fn distribution_guard_refuses_multi_rest_or_and_rest() {
        let a = atom_expr(bool_ref(1));
        let b = atom_expr(bool_ref(2));
        let multi = [atom_expr(bool_ref(3)), atom_expr(bool_ref(4))];
        assert!(distribution_would_duplicate_nontrivial_rest(2, &multi));
        let and_rest = [Expr::And(Metadata::new(), Moo::new(matrix_expr![a, b]))];
        assert!(distribution_would_duplicate_nontrivial_rest(2, &and_rest));
    }

    #[test]
    fn distribute_or_over_and_still_applies_with_one_rest_sibling() {
        let d1 = bool_ref(1);
        let d2 = bool_ref(2);
        let expr = Expr::Or(
            Metadata::new(),
            Moo::new(matrix_expr![
                Expr::And(
                    Metadata::new(),
                    Moo::new(matrix_expr![atom_expr(d1.clone()), atom_expr(d2.clone())]),
                ),
                atom_expr(d2.clone()),
            ]),
        );
        let rule = get_rule_by_name("distribute_or_over_and").expect("rule registered");
        assert!(
            rule.apply(&expr, &SymbolTable::new()).is_ok(),
            "or(and(a,b), c) must remain distributable"
        );
    }

    #[test]
    fn distribute_or_over_and_refuses_or_of_many_ands() {
        let a = atom_expr(bool_ref(1));
        let b = atom_expr(bool_ref(2));
        let c = atom_expr(bool_ref(3));
        let d = atom_expr(bool_ref(4));
        // Two and-disjuncts: rest is a single And, which must still be refused.
        let expr = Expr::Or(
            Metadata::new(),
            Moo::new(matrix_expr![
                Expr::And(Metadata::new(), Moo::new(matrix_expr![a, b])),
                Expr::And(Metadata::new(), Moo::new(matrix_expr![c, d])),
            ]),
        );
        let rule = get_rule_by_name("distribute_or_over_and").expect("rule registered");
        let err = rule.apply(&expr, &SymbolTable::new()).unwrap_err();
        assert!(
            matches!(err, ApplicationError::RuleNotApplicable),
            "or(and(a,b), and(c,d)) must not CNF-expand"
        );
    }
}

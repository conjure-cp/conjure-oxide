use std::collections::{HashSet, VecDeque};

use conjure_macros::register_rule;
use itertools::iproduct;
use uniplate::Biplate;

use crate::rule_engine::{ApplicationResult, Reduction};
use crate::Model;
use crate::{
    ast::{Atom, Expression as Expr, Literal as Lit, Literal::*},
    metadata::Metadata,
};

#[register_rule(("Base",9000))]
fn partial_evaluator(expr: &Expr, _: &Model) -> ApplicationResult {
    use conjure_core::rule_engine::ApplicationError::RuleNotApplicable;
    use Expr::*;

    // NOTE: If nothing changes, we must return RuleNotApplicable, or the rewriter will try this
    // rule infinitely!
    // This is why we always check whether we found a constant or not.
    match expr.clone() {
        Bubble(_, _, _) => Err(RuleNotApplicable),
        Atomic(_, _) => Err(RuleNotApplicable),
        Abs(m, e) => match *e {
            Neg(_, inner) => Ok(Reduction::pure(Abs(m, inner))),
            _ => Err(RuleNotApplicable),
        },
        Sum(m, vec) => {
            let mut acc = 0;
            let mut n_consts = 0;
            let mut new_vec: Vec<Expr> = Vec::new();
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Int(x))) = expr {
                    acc += x;
                    n_consts += 1;
                } else {
                    new_vec.push(expr);
                }
            }
            if acc != 0 {
                new_vec.push(Expr::Atomic(Default::default(), Atom::Literal(Int(acc))));
            }

            if n_consts <= 1 {
                Err(RuleNotApplicable)
            } else {
                Ok(Reduction::pure(Sum(m, new_vec)))
            }
        }

        Product(m, vec) => {
            let mut acc = 1;
            let mut n_consts = 0;
            let mut new_vec: Vec<Expr> = Vec::new();
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Int(x))) = expr {
                    acc *= x;
                    n_consts += 1;
                } else {
                    new_vec.push(expr);
                }
            }

            if n_consts == 0 {
                return Err(RuleNotApplicable);
            }

            new_vec.push(Expr::Atomic(Default::default(), Atom::Literal(Int(acc))));
            let new_product = Product(m, new_vec);

            if acc == 0 {
                // if safe, 0 * exprs ~> 0
                // otherwise, just return 0* exprs
                if new_product.is_safe() {
                    Ok(Reduction::pure(Expr::Atomic(
                        Default::default(),
                        Atom::Literal(Int(0)),
                    )))
                } else {
                    Ok(Reduction::pure(new_product))
                }
            } else if n_consts == 1 {
                // acc !=0, only one constant
                Err(RuleNotApplicable)
            } else {
                // acc !=0, multiple constants found
                Ok(Reduction::pure(new_product))
            }
        }

        Min(m, vec) => {
            let mut acc: Option<i32> = None;
            let mut n_consts = 0;
            let mut new_vec: Vec<Expr> = Vec::new();
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Int(x))) = expr {
                    n_consts += 1;
                    acc = match acc {
                        Some(i) => {
                            if i > x {
                                Some(x)
                            } else {
                                Some(i)
                            }
                        }
                        None => Some(x),
                    };
                } else {
                    new_vec.push(expr);
                }
            }

            if let Some(i) = acc {
                new_vec.push(Expr::Atomic(Default::default(), Atom::Literal(Int(i))));
            }

            if n_consts <= 1 {
                Err(RuleNotApplicable)
            } else {
                Ok(Reduction::pure(Min(m, new_vec)))
            }
        }

        Max(m, vec) => {
            let mut acc: Option<i32> = None;
            let mut n_consts = 0;
            let mut new_vec: Vec<Expr> = Vec::new();
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Int(x))) = expr {
                    n_consts += 1;
                    acc = match acc {
                        Some(i) => {
                            if i < x {
                                Some(x)
                            } else {
                                Some(i)
                            }
                        }
                        None => Some(x),
                    };
                } else {
                    new_vec.push(expr);
                }
            }

            if let Some(i) = acc {
                new_vec.push(Expr::Atomic(Default::default(), Atom::Literal(Int(i))));
            }

            if n_consts <= 1 {
                Err(RuleNotApplicable)
            } else {
                Ok(Reduction::pure(Max(m, new_vec)))
            }
        }
        Not(_, _) => Err(RuleNotApplicable),
        Or(m, terms) => {
            let mut has_changed = false;

            // 2. boolean literals
            let mut new_terms = vec![];
            for expr in terms {
                if let Expr::Atomic(_, Atom::Literal(Bool(x))) = expr {
                    has_changed = true;

                    // true ~~> entire or is true
                    // false ~~> remove false from the or
                    if x {
                        return Ok(Reduction::pure(true.into()));
                    }
                } else {
                    new_terms.push(expr);
                }
            }

            // 2. check pairwise tautologies.
            if check_pairwise_or_tautologies(&new_terms) {
                return Ok(Reduction::pure(true.into()));
            }

            // 3. empty or ~~> false
            if new_terms.is_empty() {
                return Ok(Reduction::pure(false.into()));
            }

            if !has_changed {
                return Err(RuleNotApplicable);
            }

            Ok(Reduction::pure(Or(m, new_terms)))
        }
        And(_, vec) => {
            let mut new_vec: Vec<Expr> = Vec::new();
            let mut has_const: bool = false;
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Bool(x))) = expr {
                    has_const = true;
                    if !x {
                        return Ok(Reduction::pure(Atomic(
                            Default::default(),
                            Atom::Literal(Bool(false)),
                        )));
                    }
                } else {
                    new_vec.push(expr);
                }
            }

            if !has_const {
                Err(RuleNotApplicable)
            } else {
                Ok(Reduction::pure(
                    expr.with_children_bi(VecDeque::from([new_vec])),
                ))
            }
        }

        // similar to And, but booleans are returned wrapped in Root.
        Root(_, vec) => {
            // root([true]) / root([false]) are already evaluated
            if vec.len() < 2 {
                return Err(RuleNotApplicable);
            }

            let mut new_vec: Vec<Expr> = Vec::new();
            let mut has_const: bool = false;
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Bool(x))) = expr {
                    has_const = true;
                    if !x {
                        return Ok(Reduction::pure(Root(
                            Metadata::new(),
                            vec![Atomic(Default::default(), Atom::Literal(Bool(false)))],
                        )));
                    }
                } else {
                    new_vec.push(expr);
                }
            }

            if !has_const {
                Err(RuleNotApplicable)
            } else {
                if new_vec.is_empty() {
                    new_vec.push(true.into());
                }
                Ok(Reduction::pure(
                    expr.with_children_bi(VecDeque::from([new_vec])),
                ))
            }
        }
        Imply(_m, x, y) => {
            if let Expr::Atomic(_, Atom::Literal(Lit::Bool(x))) = *x {
                if x {
                    // (true) -> y ~~> y
                    return Ok(Reduction::pure(*y));
                } else {
                    // (false) -> y ~~> true
                    return Ok(Reduction::pure(Expr::Atomic(Metadata::new(), true.into())));
                }
            };

            // reflexivity: p -> p ~> true

            // instead of checking syntactic equivalence of a possibly deep expression,
            // let identical-CSE turn them into identical variables first. Then, check if they are
            // identical variables.

            if x.identical_atom_to(y.as_ref()) {
                return Ok(Reduction::pure(true.into()));
            }

            Err(RuleNotApplicable)
        }
        Eq(_, _, _) => Err(RuleNotApplicable),
        Neq(_, _, _) => Err(RuleNotApplicable),
        Geq(_, _, _) => Err(RuleNotApplicable),
        Leq(_, _, _) => Err(RuleNotApplicable),
        Gt(_, _, _) => Err(RuleNotApplicable),
        Lt(_, _, _) => Err(RuleNotApplicable),
        SafeDiv(_, _, _) => Err(RuleNotApplicable),
        UnsafeDiv(_, _, _) => Err(RuleNotApplicable),
        AllDiff(m, vec) => {
            let mut consts: HashSet<i32> = HashSet::new();

            // check for duplicate constant values which would fail the constraint
            for expr in &vec {
                if let Expr::Atomic(_, Atom::Literal(Int(x))) = expr {
                    if !consts.insert(*x) {
                        return Ok(Reduction::pure(Expr::Atomic(m, Atom::Literal(Bool(false)))));
                    }
                }
            }

            // nothing has changed
            Err(RuleNotApplicable)
        }
        Neg(_, _) => Err(RuleNotApplicable),
        AuxDeclaration(_, _, _) => Err(RuleNotApplicable),
        UnsafeMod(_, _, _) => Err(RuleNotApplicable),
        SafeMod(_, _, _) => Err(RuleNotApplicable),
        UnsafePow(_, _, _) => Err(RuleNotApplicable),
        SafePow(_, _, _) => Err(RuleNotApplicable),
        Minus(_, _, _) => Err(RuleNotApplicable),

        // As these are in a low level solver form, I'm assuming that these have already been
        // simplified and partially evaluated.
        FlatAbsEq(_, _, _) => Err(RuleNotApplicable),
        FlatIneq(_, _, _, _) => Err(RuleNotApplicable),
        FlatMinusEq(_, _, _) => Err(RuleNotApplicable),
        FlatProductEq(_, _, _, _) => Err(RuleNotApplicable),
        FlatSumLeq(_, _, _) => Err(RuleNotApplicable),
        FlatSumGeq(_, _, _) => Err(RuleNotApplicable),
        FlatWatchedLiteral(_, _, _) => Err(RuleNotApplicable),
        FlatWeightedSumLeq(_, _, _, _) => Err(RuleNotApplicable),
        FlatWeightedSumGeq(_, _, _, _) => Err(RuleNotApplicable),
        MinionDivEqUndefZero(_, _, _, _) => Err(RuleNotApplicable),
        MinionModuloEqUndefZero(_, _, _, _) => Err(RuleNotApplicable),
        MinionPow(_, _, _, _) => Err(RuleNotApplicable),
        MinionReify(_, _, _) => Err(RuleNotApplicable),
        MinionReifyImply(_, _, _) => Err(RuleNotApplicable),
    }
}

/// Checks for tautologies involving pairs of terms inside an or, returning true if one is found.
///
/// This applies the following rules:
///
/// ```text
/// (p->q) \/ (q->p) ~> true    [totality of implication]
/// (p->q) \/ (p-> !q) ~> true  [conditional excluded middle]
/// ```
///
fn check_pairwise_or_tautologies(or_terms: &[Expr]) -> bool {
    // Collect terms that are structurally identical to the rule input.
    // Then, try the rules on these terms, also checking the other conditions of the rules.

    // stores (p,q) in p -> q
    let mut p_implies_q: Vec<(&Expr, &Expr)> = vec![];

    // stores (p,q) in p -> !q
    let mut p_implies_not_q: Vec<(&Expr, &Expr)> = vec![];

    for term in or_terms.iter() {
        if let Expr::Imply(_, p, q) = term {
            // we use identical_atom_to for equality later on, so these sets are mutually exclusive.
            //
            // in general however, p -> !q would be in p_implies_q as (p,!q)
            if let Expr::Not(_, q_1) = q.as_ref() {
                p_implies_not_q.push((p.as_ref(), q_1.as_ref()));
            } else {
                p_implies_q.push((p.as_ref(), q.as_ref()));
            }
        }
    }

    // `(p->q) \/ (q->p) ~> true    [totality of implication]`
    for ((p1, q1), (q2, p2)) in iproduct!(p_implies_q.iter(), p_implies_q.iter()) {
        if p1.identical_atom_to(p2) && q1.identical_atom_to(q2) {
            return true;
        }
    }

    // `(p->q) \/ (p-> !q) ~> true`    [conditional excluded middle]
    for ((p1, q1), (p2, q2)) in iproduct!(p_implies_q.iter(), p_implies_not_q.iter()) {
        if p1.identical_atom_to(p2) && q1.identical_atom_to(q2) {
            return true;
        }
    }

    false
}

use std::collections::HashSet;

use conjure_macros::register_rule;

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
        Or(m, vec) => {
            let mut new_vec: Vec<Expr> = Vec::new();
            let mut has_const: bool = false;
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Bool(x))) = expr {
                    has_const = true;
                    if x {
                        return Ok(Reduction::pure(Atomic(
                            Default::default(),
                            Atom::Literal(Bool(true)),
                        )));
                    }
                } else {
                    new_vec.push(expr);
                }
            }

            if !has_const {
                Err(RuleNotApplicable)
            } else {
                Ok(Reduction::pure(Or(m, new_vec)))
            }
        }
        And(m, vec) => {
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
                Ok(Reduction::pure(And(m, new_vec)))
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

            let x: &Atom = (&*x).try_into().or(Err(RuleNotApplicable))?;
            let y: &Atom = (&*y).try_into().or(Err(RuleNotApplicable))?;

            if x == y {
                return Ok(Reduction::pure(Expr::Atomic(Metadata::new(), true.into())));
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
        MinionReify(_, _, _) => Err(RuleNotApplicable),
    }
}

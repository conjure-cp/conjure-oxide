use std::collections::HashSet;

use conjure_macros::register_rule;

use crate::ast::{Atom, Expression as Expr, Literal::*};
use crate::rule_engine::{ApplicationResult, Reduction};
use crate::Model;

use super::utils::ToAuxVarOutput;

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
        Eq(_, _, _) => Err(RuleNotApplicable),
        Neq(_, _, _) => Err(RuleNotApplicable),
        Geq(_, _, _) => Err(RuleNotApplicable),
        Leq(_, _, _) => Err(RuleNotApplicable),
        Gt(_, _, _) => Err(RuleNotApplicable),
        Lt(_, _, _) => Err(RuleNotApplicable),
        SafeDiv(_, _, _) => Err(RuleNotApplicable),
        UnsafeDiv(_, _, _) => Err(RuleNotApplicable),
        SumEq(m, vec, eq) => {
            let mut acc = 0;
            let mut new_vec: Vec<Expr> = Vec::new();
            let mut n_consts = 0;
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Int(x))) = expr {
                    n_consts += 1;
                    acc += x;
                } else {
                    new_vec.push(expr);
                }
            }

            if let Expr::Atomic(_, Atom::Literal(Int(x))) = *eq {
                if acc != 0 {
                    // when rhs is a constant, move lhs constants to rhs
                    return Ok(Reduction::pure(SumEq(
                        m,
                        new_vec,
                        Box::new(Expr::Atomic(
                            Default::default(),
                            Atom::Literal(Int(x - acc)),
                        )),
                    )));
                }
            } else if acc != 0 {
                new_vec.push(Expr::Atomic(Default::default(), Atom::Literal(Int(acc))));
            }

            if n_consts <= 1 {
                Err(RuleNotApplicable)
            } else {
                Ok(Reduction::pure(SumEq(m, new_vec, eq)))
            }
        }
        SumGeq(m, vec, geq) => {
            let mut acc = 0;
            let mut new_vec: Vec<Expr> = Vec::new();
            let mut n_consts = 0;
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Int(x))) = expr {
                    n_consts += 1;
                    acc += x;
                } else {
                    new_vec.push(expr);
                }
            }

            if let Expr::Atomic(_, Atom::Literal(Int(x))) = *geq {
                if acc != 0 {
                    // when rhs is a constant, move lhs constants to rhs
                    return Ok(Reduction::pure(SumGeq(
                        m,
                        new_vec,
                        Box::new(Expr::Atomic(
                            Default::default(),
                            Atom::Literal(Int(x - acc)),
                        )),
                    )));
                }
            } else if acc != 0 {
                new_vec.push(Expr::Atomic(Default::default(), Atom::Literal(Int(acc))));
            }

            if n_consts <= 1 {
                Err(RuleNotApplicable)
            } else {
                Ok(Reduction::pure(SumGeq(m, new_vec, geq)))
            }
        }
        SumLeq(m, vec, leq) => {
            let mut acc = 0;
            let mut new_vec: Vec<Expr> = Vec::new();
            let mut n_consts = 0;
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Int(x))) = expr {
                    n_consts += 1;
                    acc += x;
                } else {
                    new_vec.push(expr);
                }
            }

            if let Expr::Atomic(_, Atom::Literal(Int(x))) = *leq {
                // when rhs is a constant, move lhs constants to rhs
                if acc != 0 {
                    return Ok(Reduction::pure(SumLeq(
                        m,
                        new_vec,
                        Box::new(Expr::Atomic(
                            Default::default(),
                            Atom::Literal(Int(x - acc)),
                        )),
                    )));
                }
            } else if acc != 0 {
                new_vec.push(Expr::Atomic(Default::default(), Atom::Literal(Int(acc))));
            }

            if n_consts <= 1 {
                Err(RuleNotApplicable)
            } else {
                Ok(Reduction::pure(SumLeq(m, new_vec, leq)))
            }
        }
        DivEqUndefZero(_, _, _, _) => Err(RuleNotApplicable),
        Ineq(_, _, _, _) => Err(RuleNotApplicable),
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
        WatchedLiteral(_, _, _) => Err(RuleNotApplicable),
        Reify(_, _, _) => Err(RuleNotApplicable),
        AuxDeclaration(_, _, _) => Err(RuleNotApplicable),
        UnsafeMod(_, a, b) => Err(RuleNotApplicable),
        SafeMod(_, a, b) => Err(RuleNotApplicable),
        ModuloEqUndefZero(_, _, _, _) => Err(RuleNotApplicable),
    }
}

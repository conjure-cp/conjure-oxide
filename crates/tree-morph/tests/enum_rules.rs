//! These tests use a simple constant expression tree to demonstrate the use of the `gen_reduce` crate.

use tree_morph::prelude::*;
use uniplate::Uniplate;

#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate()]
enum Expr {
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Val(i32),
}

fn rule_eval_add(_: &mut Commands<Expr, Meta>, expr: &Expr, _: &Meta) -> Option<Expr> {
    match expr {
        Expr::Add(a, b) => match (a.as_ref(), b.as_ref()) {
            (Expr::Val(x), Expr::Val(y)) => Some(Expr::Val(x + y)),
            _ => None,
        },
        _ => None,
    }
}

fn rule_eval_mul(_: &mut Commands<Expr, Meta>, expr: &Expr, _: &Meta) -> Option<Expr> {
    match expr {
        Expr::Mul(a, b) => match (a.as_ref(), b.as_ref()) {
            (Expr::Val(x), Expr::Val(y)) => Some(Expr::Val(x * y)),
            _ => None,
        },
        _ => None,
    }
}

enum MyRule {
    EvalAdd,
    EvalMul,
}

impl Rule<Expr, Meta> for MyRule {
    fn apply(&self, cmd: &mut Commands<Expr, Meta>, expr: &Expr, meta: &Meta) -> Option<Expr> {
        cmd.mut_meta(Box::new(|m: &mut Meta| m.num_applications += 1)); // Only applied if successful
        match self {
            MyRule::EvalAdd => rule_eval_add(cmd, expr, meta),
            MyRule::EvalMul => rule_eval_mul(cmd, expr, meta),
        }
    }
}
struct Meta {
    num_applications: u32,
}

#[test]
fn single_var() {
    let expr = Expr::Val(42);
    let meta = Meta {
        num_applications: 0,
    };
    let (expr, meta) = morph(
        vec![vec![MyRule::EvalAdd, MyRule::EvalMul]],
        select_first,
        expr,
        meta,
    );
    assert_eq!(expr, Expr::Val(42));
    assert_eq!(meta.num_applications, 0);
}

#[test]
fn add_zero() {
    let expr = Expr::Add(Box::new(Expr::Val(0)), Box::new(Expr::Val(42)));
    let meta = Meta {
        num_applications: 0,
    };
    let (expr, meta) = morph(
        vec![vec![MyRule::EvalAdd, MyRule::EvalMul]],
        select_first,
        expr,
        meta,
    );
    assert_eq!(expr, Expr::Val(42));
    assert_eq!(meta.num_applications, 1);
}

#[test]
fn mul_one() {
    let expr = Expr::Mul(Box::new(Expr::Val(1)), Box::new(Expr::Val(42)));
    let meta = Meta {
        num_applications: 0,
    };
    let (expr, meta) = morph(
        vec![vec![MyRule::EvalAdd, MyRule::EvalMul]],
        select_first,
        expr,
        meta,
    );
    assert_eq!(expr, Expr::Val(42));
    assert_eq!(meta.num_applications, 1);
}

#[test]
fn eval_add() {
    let expr = Expr::Add(Box::new(Expr::Val(1)), Box::new(Expr::Val(2)));
    let meta = Meta {
        num_applications: 0,
    };
    let (expr, meta) = morph(
        vec![vec![MyRule::EvalAdd, MyRule::EvalMul]],
        select_first,
        expr,
        meta,
    );
    assert_eq!(expr, Expr::Val(3));
    assert_eq!(meta.num_applications, 1);
}

#[test]
fn eval_nested() {
    let expr = Expr::Mul(
        Box::new(Expr::Add(Box::new(Expr::Val(1)), Box::new(Expr::Val(2)))),
        Box::new(Expr::Val(3)),
    );
    let meta = Meta {
        num_applications: 0,
    };
    let (expr, meta) = morph(
        vec![vec![MyRule::EvalAdd, MyRule::EvalMul]],
        select_first,
        expr,
        meta,
    );
    assert_eq!(expr, Expr::Val(9));
    assert_eq!(meta.num_applications, 2);
}

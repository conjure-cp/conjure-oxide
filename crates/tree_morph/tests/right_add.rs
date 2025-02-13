use tree_morph::prelude::*;
use uniplate::derive::Uniplate;

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
        cmd.mut_meta(|m| m.num_applications += 1); // Only applied if successful
        match self {
            MyRule::EvalAdd => rule_eval_add(cmd, expr, meta),
            MyRule::EvalMul => rule_eval_mul(cmd, expr, meta),
        }
    }
}

struct Meta {
    num_applications: u32,
}

fn val(n: i32) -> Box<Expr> {
    Box::new(Expr::Val(n))
}

fn add(lhs: Box<Expr>, rhs: Box<Expr>) -> Box<Expr> {
    Box::new(Expr::Add(lhs, rhs))
}

fn mul(lhs: Box<Expr>, rhs: Box<Expr>) -> Box<Expr> {
    Box::new(Expr::Mul(lhs, rhs))
}

fn nested_addition(n: i32) -> Box<Expr> {
    if n == 1 {
        val(1)
    } else {
        add(nested_addition(n - 1), val(1))
    }
}

#[test]
fn eval_nested() {
    let base: i32 = 2;
    let expr = *nested_addition(base.pow(7));
    let meta = Meta {
        num_applications: 0,
    };
    let (expr, meta) = morph(
        vec![vec![MyRule::EvalAdd, MyRule::EvalMul]],
        select_first,
        expr,
        meta,
    );
    assert_eq!(expr, Expr::Val(32));
}

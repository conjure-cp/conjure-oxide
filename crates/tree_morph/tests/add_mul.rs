//! These tests use a simple constant expression tree to demonstrate the use of the `gen_reduce` crate.

use tree_morph::prelude::*;
use uniplate::derive::Uniplate;

#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate()]
enum Expr {
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Val(i32),
}

struct Meta {
    num_applications: u32,
}

enum ReductionRule {
    AddZero,
    MulOne,
    Eval,
}

impl Rule<Expr, Meta> for ReductionRule {
    fn apply(&self, cmd: &mut Commands<Expr, Meta>, expr: &Expr, _: &Meta) -> Option<Expr> {
        use Expr::*;
        use ReductionRule::*;

        let result = match self {
            AddZero => match expr {
                Add(a, b) if matches!(a.as_ref(), Val(0)) => Some(*b.clone()),
                Add(a, b) if matches!(b.as_ref(), Val(0)) => Some(*a.clone()),
                _ => None,
            },
            MulOne => match expr {
                Mul(a, b) if matches!(a.as_ref(), Val(1)) => Some(*b.clone()),
                Mul(a, b) if matches!(b.as_ref(), Val(1)) => Some(*a.clone()),
                _ => None,
            },
            Eval => match expr {
                Add(a, b) => match (a.as_ref(), b.as_ref()) {
                    (Val(x), Val(y)) => Some(Val(x + y)),
                    _ => None,
                },
                Mul(a, b) => match (a.as_ref(), b.as_ref()) {
                    (Val(x), Val(y)) => Some(Val(x * y)),
                    _ => None,
                },
                _ => None,
            },
        };

        if result.is_some() {
            cmd.mut_meta(|m| m.num_applications += 1);
        }
        result
    }
}

#[test]
fn test_single_var() {
    let expr = Expr::Val(42);
    let meta = Meta {
        num_applications: 0,
    };
    let (expr, meta) = reduce_with_rules(&[ReductionRule::Eval], select_first, expr, meta);
    assert_eq!(expr, Expr::Val(42));
    assert_eq!(meta.num_applications, 0);
}

#[test]
fn test_add_zero() {
    let expr = Expr::Add(Box::new(Expr::Val(0)), Box::new(Expr::Val(42)));
    let meta = Meta {
        num_applications: 0,
    };
    let (expr, meta) = reduce_with_rules(&[ReductionRule::AddZero], select_first, expr, meta);
    assert_eq!(expr, Expr::Val(42));
    assert_eq!(meta.num_applications, 1);
}

#[test]
fn test_mul_one() {
    let expr = Expr::Mul(Box::new(Expr::Val(1)), Box::new(Expr::Val(42)));
    let meta = Meta {
        num_applications: 0,
    };
    let (expr, meta) = reduce_with_rules(&[ReductionRule::MulOne], select_first, expr, meta);
    assert_eq!(expr, Expr::Val(42));
    assert_eq!(meta.num_applications, 1);
}

#[test]
fn test_eval_add() {
    let expr = Expr::Add(Box::new(Expr::Val(1)), Box::new(Expr::Val(2)));
    let meta = Meta {
        num_applications: 0,
    };
    let (expr, meta) = reduce_with_rules(&[ReductionRule::Eval], select_first, expr, meta);
    assert_eq!(expr, Expr::Val(3));
    assert_eq!(meta.num_applications, 1);
}

#[test]
fn test_eval_nested() {
    let expr = Expr::Mul(
        Box::new(Expr::Add(Box::new(Expr::Val(1)), Box::new(Expr::Val(2)))),
        Box::new(Expr::Val(3)),
    );
    let meta = Meta {
        num_applications: 0,
    };
    let (expr, meta) = reduce_with_rules(&[ReductionRule::Eval], select_first, expr, meta);
    assert_eq!(expr, Expr::Val(9));
    assert_eq!(meta.num_applications, 2);
}

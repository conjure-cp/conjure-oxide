///This is a simple benchmark that tests how long it takes tree-morph to evaluate (1+(1+...)).
///This benchmark is designed to be compared with right_add.
///
use criterion::{Criterion, criterion_group, criterion_main};
use tree_morph::prelude::*;
use uniplate::Uniplate;

#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate()]
enum Expr {
    Add(Box<Expr>, Box<Expr>),
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

#[derive(Clone)]
enum MyRule {
    EvalAdd,
}

impl Rule<Expr, Meta> for MyRule {
    fn apply(&self, cmd: &mut Commands<Expr, Meta>, expr: &Expr, meta: &Meta) -> Option<Expr> {
        cmd.mut_meta(Box::new(|m: &mut Meta| m.num_applications += 1)); // Only applied if successful
        match self {
            MyRule::EvalAdd => rule_eval_add(cmd, expr, meta),
        }
    }
}

#[derive(Clone)]
struct Meta {
    num_applications: u32,
}

fn val(n: i32) -> Box<Expr> {
    Box::new(Expr::Val(n))
}

fn add(lhs: Box<Expr>, rhs: Box<Expr>) -> Box<Expr> {
    Box::new(Expr::Add(lhs, rhs))
}

fn nested_addition(n: i32) -> Box<Expr> {
    if n == 1 {
        val(1)
    } else {
        add(val(1), nested_addition(n - 1))
    }
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let base: i32 = 2;
    let expr = *nested_addition(base.pow(5));
    let rules = vec![vec![MyRule::EvalAdd]];

    c.bench_function("left_add", |b| {
        b.iter(|| {
            let meta = Meta {
                num_applications: 0,
            };
            morph(
                rules.clone(),
                select_first,
                std::hint::black_box(expr.clone()),
                meta,
            )
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

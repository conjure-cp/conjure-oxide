///Added 4 extra rules (that never apply) to left_add, showing a performance cost of > +400%
///Good optimisations will meant that this cost is vastly reduced (I am not sure by how much, but I think < +100% makes sense)
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
    Fee,
    Fi,
    Fo,
    Fum,
}

impl Rule<Expr, Meta> for MyRule {
    fn apply(&self, cmd: &mut Commands<Expr, Meta>, expr: &Expr, meta: &Meta) -> Option<Expr> {
        cmd.mut_meta(Box::new(|m: &mut Meta| m.num_applications += 1)); // Only applied if successful
        match self {
            MyRule::EvalAdd => rule_eval_add(cmd, expr, meta),
            MyRule::Fee => None,
            MyRule::Fi => None,
            MyRule::Fo => None,
            MyRule::Fum => None,
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
    let rules = vec![
        vec![MyRule::Fi],
        vec![MyRule::Fee],
        vec![MyRule::Fo],
        vec![MyRule::Fum],
        vec![MyRule::EvalAdd],
    ];

    c.bench_function("left_add_hard", |b| {
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

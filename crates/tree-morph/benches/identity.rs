///The following test is designed to test how long tree traversal takes.
///There is one rule, that does nothing. We create trees of a variable depth.
use criterion::{Criterion, criterion_group, criterion_main};
use tree_morph::prelude::*;
use uniplate::Uniplate;

#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate()]
enum Expr {
    Branch(Box<Expr>, Box<Expr>),
    Val(i32),
}
struct Meta {}
fn do_nothing(_: &mut Commands<Expr, Meta>, _: &Expr, _: &Meta) -> Option<Expr> {
    None
}

fn construct_tree(n: i32) -> Box<Expr> {
    if n == 1 {
        Box::new(Expr::Val(0))
    } else {
        Box::new(Expr::Branch(Box::new(Expr::Val(0)), construct_tree(n - 1)))
    }
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let base: i32 = 2;
    let expr = *construct_tree(base.pow(5));
    let rules = vec![vec![do_nothing]];

    c.bench_function("Identity", |b| {
        b.iter(|| {
            morph(
                std::hint::black_box(rules.clone()),
                select_first,
                std::hint::black_box(expr.clone()),
                Meta {},
            )
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

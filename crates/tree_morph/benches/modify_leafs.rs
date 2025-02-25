///This benchmark aims to assess how compute-heavy modifying all the nodes is.
///A tree of depth n with n children will be created, with the only rule being a modification 0->1.
///This benchmark will assess how efficient tree-updating (which is not done in place) is.
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tree_morph::prelude::*;
use uniplate::derive::Uniplate;

#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate()]
enum Expr {
    Branch(Box<Expr>, Box<Expr>),
    Val(i32),
}

struct Meta {} // not relevant for this benchmark

fn zero_to_one(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
    if let Expr::Val(a) = subtree {
        if let 0 = *a {
            return Some(Expr::Val(1));
        }
    }
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
    let rules = vec![vec![zero_to_one]];

    c.bench_function("Modify_leafs", |b| {
        b.iter(|| {
            morph(
                black_box(rules.clone()),
                select_first,
                black_box(expr.clone()),
                Meta {},
            )
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

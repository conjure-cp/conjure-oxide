use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use std::hint::black_box;
use tree_morph::prelude::*;
use uniplate::derive::Uniplate;

#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate()]
enum Expr {
    Branch(Box<Expr>, Box<Expr>),
    Val(i32),
}

fn construct_tree(n: i32) -> Box<Expr> {
    if n == 1 {
        Box::new(Expr::Val(0))
    } else {
        Box::new(Expr::Branch(Box::new(Expr::Val(0)), construct_tree(n - 1)))
    }
}

struct Meta {} // not relevant for this benchmark

fn zero_to_one(_cmds: &mut Commands<Expr, Meta>, subtree: &Expr, _meta: &Meta) -> Option<Expr> {
    if let Expr::Val(a) = subtree {
        if let 0 = *a {
            return Some(Expr::Val(1));
        }
    }
    None
}

fn generate(
    n: i32,
) -> (
    Vec<Vec<fn(&mut Commands<Expr, Meta>, &Expr, &Meta) -> Option<Expr>>>,
    Expr,
    Meta,
) {
    let expression = construct_tree(n);
    let rules: Vec<Vec<fn(&mut Commands<Expr, Meta>, &Expr, &Meta) -> Option<Expr>>> =
        vec![rule_fns![zero_to_one]];
    let meta = Meta {};
    (rules, *expression, meta)
}

#[library_benchmark]
#[bench::optimised(generate(50))]
fn bench_morph_op(
    data: (
        Vec<Vec<fn(&mut Commands<Expr, Meta>, &Expr, &Meta) -> Option<Expr>>>,
        Expr,
        Meta,
    ),
) -> (Expr, Meta) {
    let (rules, expression, meta) = data;
    black_box(morph(rules, select_first, expression, meta))
}

#[library_benchmark]
#[bench::unpoptimised(generate(50))]
fn bench_morph_unop(
    data: (
        Vec<Vec<fn(&mut Commands<Expr, Meta>, &Expr, &Meta) -> Option<Expr>>>,
        Expr,
        Meta,
    ),
) -> (Expr, Meta) {
    let (rules, expression, meta) = data;
    black_box(morph_not_opt(rules, select_first, expression, meta))
}

library_benchmark_group!(
    name = bench_testing_group;
    benchmarks = bench_morph_op, bench_morph_unop
);

main!(library_benchmark_groups = bench_testing_group);

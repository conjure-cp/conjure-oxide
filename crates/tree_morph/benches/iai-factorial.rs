use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::hint::black_box;
use tree_morph::prelude::*;
use uniplate::derive::Uniplate;

#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate()]
enum Expr {
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Val(i32),
    Factorial(Box<Expr>),
}

fn random_exp_tree(rng: &mut StdRng, count: &mut usize, depth: usize) -> Expr {
    if depth == 0 {
        *count += 1;
        return Expr::Val(rng.random_range(1..=3));
    }

    match rng.random_range(1..=13) {
        x if (1..=4).contains(&x) => Expr::Add(
            Box::new(random_exp_tree(rng, count, depth - 1)),
            Box::new(random_exp_tree(rng, count, depth - 1)),
        ),
        x if (5..=8).contains(&x) => Expr::Mul(
            Box::new(random_exp_tree(rng, count, depth - 1)),
            Box::new(random_exp_tree(rng, count, depth - 1)),
        ),
        x if (8..=11).contains(&x) => {
            Expr::Factorial(Box::new(random_exp_tree(rng, count, depth - 1)))
        }
        _ => Expr::Val(rng.random_range(1..=10)),
    }
}
fn do_nothing(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
    None
}

fn factorial_eval(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
    if let Expr::Factorial(a) = subtree {
        if let Expr::Val(n) = *a.as_ref() {
            if n == 0 {
                return Some(Expr::Val(1));
            }
            return Some(Expr::Mul(
                Box::new(Expr::Val(n)),
                Box::new(Expr::Factorial(Box::new(Expr::Val(n - 1)))),
            ));
        }
    }
    None
}

fn rule_eval_add(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
    if let Expr::Add(a, b) = subtree {
        if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
            cmds.mut_meta(Box::new(|m: &mut Meta| m.num_applications_addition += 1));
            return Some(Expr::Val((a_v + b_v) % 10));
        }
    }
    None
}

fn rule_eval_mul(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
    if let Expr::Mul(a, b) = subtree {
        if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
            cmds.mut_meta(Box::new(|m: &mut Meta| {
                m.num_applications_multiplication += 1
            }));
            return Some(Expr::Val((a_v * b_v) % 10));
        }
    }
    None
}

#[derive(Debug)]
struct Meta {
    num_applications_addition: i32,
    num_applications_multiplication: i32,
}

fn generate(
    n: usize,
) -> (
    Vec<Vec<fn(&mut Commands<Expr, Meta>, &Expr, &Meta) -> Option<Expr>>>,
    Expr,
    Meta,
) {
    let seed = [41; 32];
    let mut rng = StdRng::from_seed(seed);
    let mut count = 0;
    let my_expression = random_exp_tree(&mut rng, &mut count, n);
    let rules = vec![
        rule_fns![do_nothing],
        rule_fns![rule_eval_add, rule_eval_mul, factorial_eval],
    ];
    let meta = Meta {
        num_applications_addition: 0,
        num_applications_multiplication: 0,
    };
    (rules, my_expression, meta)
}

#[library_benchmark]
#[bench::five(generate(5))]
#[bench::ten(generate(10))]
fn bench_morph(
    data: (
        Vec<Vec<fn(&mut Commands<Expr, Meta>, &Expr, &Meta) -> Option<Expr>>>,
        Expr,
        Meta,
    ),
) -> (Expr, Meta) {
    let (rules, expression, meta) = data;
    black_box(morph(rules, select_first, expression, meta))
}

library_benchmark_group!(
    name = bench_testing_group;
    benchmarks = bench_morph
);

main!(library_benchmark_groups = bench_testing_group);

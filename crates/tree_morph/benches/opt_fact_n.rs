///This aims to create data on the larger n behaviour of the current factorial benchmark.
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
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
        _ => Expr::Factorial(Box::new(random_exp_tree(rng, count, depth - 1))),
    }
}
fn do_nothing(_cmds: &mut Commands<Expr, Meta>, _subtree: &Expr, _meta: &Meta) -> Option<Expr> {
    None
}

fn factorial_eval(_cmds: &mut Commands<Expr, Meta>, subtree: &Expr, _meta: &Meta) -> Option<Expr> {
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

fn rule_eval_add(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, _meta: &Meta) -> Option<Expr> {
    if let Expr::Add(a, b) = subtree {
        if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
            cmds.mut_meta(Box::new(|m: &mut Meta| m.num_applications_addition += 1));
            return Some(Expr::Val((a_v + b_v) % 10));
        }
    }
    None
}

fn rule_eval_mul(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, _meta: &Meta) -> Option<Expr> {
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

fn expr_to_string(expr: &Expr) -> String {
    match expr {
        Expr::Val(n) => n.to_string(),
        Expr::Add(a, b) => format!("({} + {})", expr_to_string(a), expr_to_string(b)),
        Expr::Mul(a, b) => format!("({} * {})", expr_to_string(a), expr_to_string(b)),
        Expr::Factorial(a) => format!("{}!", expr_to_string(a)),
    }
}

#[derive(Debug)]
struct Meta {
    num_applications_addition: i32,
    num_applications_multiplication: i32,
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let seed = [41; 32];
    let mut rng = StdRng::from_seed(seed);
    let rules = vec![
        rule_fns![do_nothing],
        rule_fns![rule_eval_add, rule_eval_mul, factorial_eval],
    ];

    for n in 1..=12 {
        let mut count = 0;
        let my_expression = random_exp_tree(&mut rng, &mut count, n);
        let benchmark_id = format!("opt_factorial_{}", n);

        println!(
            "Benchmarking {} with expression: {}",
            benchmark_id,
            expr_to_string(&my_expression)
        );
        let mut group = c.benchmark_group("opt_factorial_n");
        let sample_size = match n {
            1..=8 => 50,
            _ => 10,
        };
        group.sample_size(sample_size);
        group.bench_function(benchmark_id, |b| {
            b.iter(|| {
                let meta = Meta {
                    num_applications_addition: 0,
                    num_applications_multiplication: 0,
                };
                morph(
                    black_box(rules.clone()),
                    select_first,
                    black_box(my_expression.clone()),
                    black_box(meta),
                )
            })
        });
        group.finish();
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

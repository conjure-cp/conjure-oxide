///This creates a random factorial tree of a given depth, and evaluates it via a benchmark
///Note that addition and multiplication are performed modulo 10, to avoid having large numbers
/// in the factorial. I also add in a dummy "do_nothing" rule. This gives another area where
/// an optimiser can shine.
use criterion::{Criterion, criterion_group, criterion_main};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use tree_morph::prelude::*;
use uniplate::Uniplate;

#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
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
fn do_nothing(_: &mut Commands<Expr, Meta>, _: &Expr, _: &Meta) -> Option<Expr> {
    None
}

fn factorial_eval(_: &mut Commands<Expr, Meta>, subtree: &Expr, _: &Meta) -> Option<Expr> {
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

fn rule_eval_add(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, _: &Meta) -> Option<Expr> {
    if let Expr::Add(a, b) = subtree {
        if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
            cmds.mut_meta(Box::new(|m: &mut Meta| m.num_applications_addition += 1));
            return Some(Expr::Val((a_v + b_v) % 10));
        }
    }
    None
}

fn rule_eval_mul(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, _: &Meta) -> Option<Expr> {
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

pub fn criterion_benchmark(c: &mut Criterion) {
    let seed = [41; 32];
    let mut rng = StdRng::from_seed(seed);
    let mut count = 0;

    let my_expression = random_exp_tree(&mut rng, &mut count, 10);
    let rules = vec![
        rule_fns![do_nothing],
        rule_fns![rule_eval_add, rule_eval_mul, factorial_eval],
    ];

    c.bench_function("factorial", |b| {
        b.iter(|| {
            let meta = Meta {
                num_applications_addition: 0,
                num_applications_multiplication: 0,
            };
            morph(
                std::hint::black_box(rules.clone()),
                select_first,
                std::hint::black_box(my_expression.clone()),
                std::hint::black_box(meta),
            )
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

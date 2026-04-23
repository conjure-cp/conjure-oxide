[//]: # (Author: Owain Thorp)
[//]: # (Last Updated: 27/02/2025)

Tree-morph is a library that helps you perform boilerplate-free generic tree transformations. In its current state, tree-morph is able to perform transformations in a large number of test cases, and work is now being done to try to implement Essence with tree-morph. When completed, tree-morph will be a very powerful stand-alone crate, as well as sitting at the core of ``conjure-oxide``. Despite its current power, there is a still a lot of work to be done on the ``tree-morph`` crate. It is currently in a completely unoptimised state, and will be very slow when performing on large trees with a rich rule set and rule hierarchy. In order to assess progress on new optimisations, it is essential to have a diverse list of benchmarks. All benchmarking has been done using the [criterion](https://crates.io/crates/criterion) crate. The following section outlines some of the most important benchmarks.

# Benchmarks

## identity

The ``identity benchmark`` is a test designed to capture how long one tree traversal takes. There is no metadata, and the only rule is ``do_nothing`` which is never evaluated. The helper function ``construct_tree`` creates a simple tree of variable depth. This is an important benchmark to make sure that new proposed changes do not negatively effect ``tree-morph`` traversal speed.

Filename: ``conjure-oxide/crates/tree_morph/benches/identity.rs``

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tree_morph::prelude::*;
use uniplate::derive::Uniplate;

#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate()]
enum Expr {
    Branch(Box<Expr>, Box<Expr>),
    Val(i32),
}
struct Meta {}
fn do_nothing(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
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
```

## modify_leafs

The ``modify_leafs`` benchmark is designed to capture how long it takes for a simple modification rule to be applied to all of the leaf nodes. The function ``construct_tree`` creates a tree of variable depth with all leaf nodes initialised to 0. There is no metadata and the only function ``zero_to_one`` changes a leaf node of 0 to a leaf node of 1. It should be expected that this will take a lot longer than the identity benchmark, as when a node is changed in ``tree-morph``, an entirely new tree is created.

Filename: ``conjure-oxide/crates/tree_morph/benches/modify_leafs.rs``

```rust
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
```

## factorial

The ``factorial`` benchmark is the most difficult benchmark currently made, and an improved score on factorial will be indicative of serious gains in optimisations. As the name suggests, ``factorial`` is centred around the mathematical factorial operation (``5! = 5*4*3*2*1``), which will grow the tree depth, providing a rich set of transformations for ``tree-morph`` to calculate. The tree generating function ``random_exp_tree`` here takes as input a random seed and a max depth, and generates a tree of an arithmetic expression involving values, additions, multiplications and factorials. An example of such an expression with a random seed of ``41`` and a max depth of ``5`` is shown below.

```bash
(((8 + 1!) + (1 * 3)!) + (1!! * ((2 + 1) * (1 * 1))))!
```

It is worth mentioning that this expression is extremely large due to the nested factorials, and as such we need a way of making sure that calculations are bounded. This is achieved by modding the results of any additions and multiplications in the ``rule_eval_add`` and ``rule_eval_mul`` functions by ``10``, which will bound all factorial expressions by ``10!``. The benchmark counts the number of addition and multiplication rule applications, and also has a high priority dummy rule ``do_nothing``, in order to increase the benchmark difficulty.

Filename: ``conjure-oxide/crates/tree_morph/benches/factorial.rs``

```rust
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
            cmds.mut_meta(|m| m.num_applications_addition += 1);
            return Some(Expr::Val((a_v + b_v) % 10));
        }
    }
    None
}

fn rule_eval_mul(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
    if let Expr::Mul(a, b) = subtree {
        if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
            cmds.mut_meta(|m| m.num_applications_multiplication += 1);
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
                black_box(rules.clone()),
                select_first,
                black_box(my_expression.clone()),
                black_box(meta),
            )
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
```

## left_add/left_add_hard

The benchmarks ``left_add`` and ``left_add_hard`` are two benchmarks that evaluate a very simple nested addition expression ``(1+(1+(1+...)))`` of variable depth. In its unoptimised state, all but he final two instances of a ``1`` node will undergo several superfluous rule checks. The benchmark ``left_add_hard`` also is identical to ``left_add``, except there are also four additional dummy rules, all assigned with a higher priority than the ``rule_eval_add``. As expected, this will reduce code performance by about 400% (shown later). The code for ``left_add_hard`` is shown below, with ``left_add`` being very similar.

Filename: ``conjure-oxide/crates/tree_morph/benches/left_add_hard.rs``

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tree_morph::prelude::*;
use uniplate::derive::Uniplate;

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
        cmd.mut_meta(|m| m.num_applications += 1); // Only applied if successful
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
            morph(rules.clone(), select_first, black_box(expr.clone()), meta)
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
```

## right_add

The benchmark ``right_add`` is identical to ``left_add``, except there now we are evaluating ``((...+1)+1)`` instead. This is designed to show the inherent left bias used in ``tree-morph``. Performance is a little better than ``left_add``. Due to the similarity with ``left_add``, the code is omitted.

# results

The most helpful feature that ``criterion`` produces is its automatic graphing software. Upon a run of ``cargo bench``, ``criterion`` automatically produces html reports of all benchmarks, accessible in ``conjure-oxide/target/criterion``. An example report is shown below.
<img width="1300" alt="image" src="https://github.com/user-attachments/assets/963c76eb-2d8c-4c77-8529-8b9b6bc22dc3" />
The left graph is a probability density function for the run time. It says that on average it takes ``854.26 µs`` for identity to run at a fixed depth (in this case ``2^5`` was used). The right graph is a cumulative time plot. The fact that the line is almost straight indicates that the run times are all very comparable in time. The additional stats provided are all standard statistics measurements, with MAD standing for the mean absolute deviation.

## identity vs modify_leafs

The probability density functions for ``identity`` and ``modify_leafs`` are shown below, both evaluated a tree depth of ``2^5``.
<img width="550" alt="image" src="https://github.com/user-attachments/assets/06fd9c0e-b094-4a5f-ab5e-6d88b3ace29a" />
<img width="547.5" alt="image" src="https://github.com/user-attachments/assets/2e31067d-a848-4259-bf67-bf516c70a6cf" />

As you can see, ``modify_leafs`` takes significantly longer (``2300%``) than ``identity`` on average, showing how costly even simple tree transformations take. This is most likely due to the fact that ``tree-morph`` does not edit in-place, but rather constructs a new tree after applying a transformation.

## factorial

The following is the probability density function for ``factorial``, ran at a max depth of ``10`` and a seed of ``[41; 32]``.
![image](https://github.com/user-attachments/assets/afb83193-b33c-452b-a5ba-8b9b9ae66f1a)

As you can see, even for a relatively small max depth compute is large. Also note that ``factorial`` still has a very small amount of rule groupings, and performance would likely be orders of magnitude worse if a number of rules comparable to a ``conjure`` problem was presented. This shows a significant need for optimisations, and an improved score on ``factorial`` would indicate a lot of good progress.  

## left_add vs left_add_hard

A comparison between ``left_add`` and ``left_add_hard`` is a clear demonstration of how detrimental duplicate rule checks can be for performance.

![image](https://github.com/user-attachments/assets/ae292d37-f26d-4080-9520-019f60476e93)
![image](https://github.com/user-attachments/assets/2f882085-71fc-4d13-a266-ead400db31e8)

Ignoring the differences between the shapes of the graphs (I am unsure why this is the case), we can immediately notice how the extra 4 dummy rules in ``left_add_hard`` lead to almost exactly a ``400%`` increase in performance!  

# Further work

Benchmarks are still in an early stage, and there is lots more to do. Some ideas for future work include:

- Set up some automated infrastructure for running benchmarks automatically on GitHub
- Find out scaling laws for increasing tree depth for current benchmarks
- Set up some head to head tests between two functions
- Set up counters that can measure things like node counts, maximum tree depths and other useful information
- Benchmark individual functions (such as a rule application) in order to better understand how trees grow and shrink during transformations
- Benchmark meta applications

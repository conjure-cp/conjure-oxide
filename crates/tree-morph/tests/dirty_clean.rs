use std::sync::atomic::Ordering;

use std::sync::atomic::AtomicUsize;
use tracing_subscriber;
use tree_morph::prelude::*;
use tree_morph_macros::named_rule;
use uniplate::Uniplate;

static GLOBAL_RULE_CHECKS: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate()]
enum Expr {
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Val(i32),
}

#[named_rule("Eval Add")]
fn rule_eval_add(cmd: &mut Commands<Expr, Meta>, expr: &Expr, _: &Meta) -> Option<Expr> {
    // THIS IS FOR TESTING ONLY
    // Not meant to integrated into the main code.
    GLOBAL_RULE_CHECKS.fetch_add(1, Ordering::Relaxed);

    match expr {
        Expr::Add(a, b) => match (a.as_ref(), b.as_ref()) {
            (Expr::Val(x), Expr::Val(y)) => {
                cmd.mut_meta(Box::new(|m: &mut Meta| m.num_applications += 1));
                Some(Expr::Val(x + y))
            }
            _ => None,
        },
        _ => None,
    }
}

#[named_rule("Eval Mul")]
fn rule_eval_mul(cmd: &mut Commands<Expr, Meta>, expr: &Expr, _: &Meta) -> Option<Expr> {
    // THIS IS FOR TESTING ONLY
    // Not meant to integrated into the main code.
    GLOBAL_RULE_CHECKS.fetch_add(1, Ordering::Relaxed);

    match expr {
        Expr::Mul(a, b) => match (a.as_ref(), b.as_ref()) {
            (Expr::Val(x), Expr::Val(y)) => {
                cmd.mut_meta(Box::new(|m: &mut Meta| m.num_applications += 1));
                Some(Expr::Val(x * y))
            }
            _ => None,
        },
        _ => None,
    }
}

#[derive(Debug)]
struct Meta {
    num_applications: u32,
}

#[test]
fn left_branch_clean() {
    // Initialize tracing subscriber to see trace output
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .try_init();

    // Top Level is +
    // Left Branch has two Nested Subtractions which do not have any rules
    // So atoms
    // Right brigh has Mul and Add which DO have rules
    let expr = Expr::Add(
        Box::new(Expr::Sub(
            Box::new(Expr::Val(1)),
            Box::new(Expr::Val(1)),
            // Box::new(Expr::Sub(Box::new(Expr::Val(1)), Box::new(Expr::Val(2)))),
            // Box::new(Expr::Sub(Box::new(Expr::Val(3)), Box::new(Expr::Val(10)))),
        )),
        Box::new(Expr::Mul(Box::new(Expr::Val(10)), Box::new(Expr::Val(5)))),
    );

    let meta = Meta {
        num_applications: 0,
    };

    let engine = EngineBuilder::new()
        .add_rule_group(vec![rule_eval_add(), rule_eval_mul()])
        .build();
    let (expr, meta) = engine.morph(expr, meta);

    println!("RAN TESTS");
    println!("Number of applications: {}", meta.num_applications);
    println!(
        "Number of Rule Application Checks {}",
        GLOBAL_RULE_CHECKS.load(Ordering::Relaxed)
    );
    dbg!(expr);
}

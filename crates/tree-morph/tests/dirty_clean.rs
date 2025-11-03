use std::collections::HashMap;
use std::sync::atomic::Ordering;

use std::sync::atomic::AtomicUsize;
use tracing_subscriber;
use tree_morph::prelude::*;
use tree_morph_macros::named_rule;
use uniplate::Uniplate;

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
    attempted: HashMap<String, usize>,
    applicable: HashMap<String, usize>,
    applied: HashMap<String, usize>,
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
        Box::new(Expr::Add(
            Box::new(Expr::Val(1)),
            Box::new(Expr::Val(1)),
            // Box::new(Expr::Sub(Box::new(Expr::Val(1)), Box::new(Expr::Val(2)))),
            // Box::new(Expr::Sub(Box::new(Expr::Val(3)), Box::new(Expr::Val(10)))),
        )),
        Box::new(Expr::Mul(Box::new(Expr::Val(10)), Box::new(Expr::Val(5)))),
    );

    let meta = Meta {
        num_applications: 0,
        attempted: HashMap::new(),
        applicable: HashMap::new(),
        applied: HashMap::new(),
    };

    let engine = EngineBuilder::new()
        .add_rule_group(vec![rule_eval_add(), rule_eval_mul()])
        .add_before_rule(|node, meta, rule| {
            meta.num_applications += 1;
            let attempt = meta.attempted.entry(rule.name().to_owned()).or_insert(0);
            *attempt += 1
        })
        .add_after_rule(|node, meta, rule, success| {
            if success {
                let attempt = meta.applicable.entry(rule.name().to_owned()).or_insert(0);
                *attempt += 1
            }
        })
        .build();
    let (expr, meta) = engine.morph(expr, meta);

    for name in meta.attempted.keys() {
        let attempted = *meta.attempted.get(name).unwrap();
        let applicable = *meta.applicable.get(name).unwrap_or(&0);
        println!("Rule: {}: {}/{} ({:.2}%)",name, applicable, attempted, applicable / attempted)
    }

    dbg!(expr);
}

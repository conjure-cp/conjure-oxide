use std::{collections::HashMap, ops::DerefMut, sync::atomic::{AtomicU64, Ordering}};

use tree_morph::{
    cache::{HashMapCache, NoCache, StdHashKey},
    prelude::*,
};
use tree_morph_macros::named_rule;
use uniplate::Uniplate;

#[derive(Debug, Clone, PartialEq, Eq, Uniplate, Hash)]
#[uniplate()]
enum Expr {
    Triple(Box<Expr>, Box<Expr>, Box<Expr>),
    Quad(Box<Expr>, Box<Expr>, Box<Expr>, Box<Expr>),
    A,
    B,
    C,
    D,
}

#[derive(Debug)]
struct Meta {
    attempts: HashMap<String, usize>,
    applied: HashMap<String, usize>,
}

#[named_rule("a->b")]
fn a_to_b(cmd: &mut Commands<Expr, Meta>, expr: &Expr, _meta: &Meta) -> Option<Expr> {
    cmd.mut_meta(Box::new(|m| {
        *m.attempts.entry("a->b".into()).or_default().deref_mut() += 1
    }));
    match expr {
        Expr::A => {
            cmd.mut_meta(Box::new(|m| {
                *m.applied.entry("a->b".into()).or_default().deref_mut() += 1
            }));
            Some(Expr::B)
        }
        _ => None,
    }
}

#[named_rule("b->c")]
fn b_to_c(cmd: &mut Commands<Expr, Meta>, expr: &Expr, _meta: &Meta) -> Option<Expr> {
    cmd.mut_meta(Box::new(|m| {
        *m.attempts.entry("b->c".into()).or_default().deref_mut() += 1
    }));
    match expr {
        Expr::B => {
            cmd.mut_meta(Box::new(|m| {
                *m.applied.entry("b->c".into()).or_default().deref_mut() += 1
            }));
            Some(Expr::C)
        }
        _ => None,
    }
}

#[named_rule("c->d")]
fn c_to_d(cmd: &mut Commands<Expr, Meta>, expr: &Expr, _meta: &Meta) -> Option<Expr> {
    cmd.mut_meta(Box::new(|m| {
        *m.attempts.entry("c->d".into()).or_default().deref_mut() += 1
    }));
    match expr {
        Expr::C => {
            cmd.mut_meta(Box::new(|m| {
                *m.applied.entry("c->d".into()).or_default().deref_mut() += 1
            }));
            Some(Expr::D)
        }
        _ => None,
    }
}

fn setup() -> (
    Meta,
    Engine<
        Expr,
        Meta,
        NamedRule<
            for<'a, 'b, 'c> fn(
                &'a mut tree_morph::commands::Commands<Expr, Meta>,
                &'b Expr,
                &'c Meta,
            ) -> Option<Expr>,
        >,
        HashMapCache<Expr>,
    >,
) {
    let meta = Meta {
        applied: HashMap::new(),
        attempts: HashMap::new(),
    };
    let engine = EngineBuilder::new()
        .add_rule_group(vec![a_to_b, b_to_c, c_to_d])
        .add_cacher(HashMapCache::<_, StdHashKey>::new())
        .build();
    (meta, engine)
}

fn setup_nocache() -> (
    Meta,
    Engine<
        Expr,
        Meta,
        NamedRule<
            for<'a, 'b, 'c> fn(
                &'a mut tree_morph::commands::Commands<Expr, Meta>,
                &'b Expr,
                &'c Meta,
            ) -> Option<Expr>,
        >,
        NoCache,
    >,
) {
    let meta = Meta {
        applied: HashMap::new(),
        attempts: HashMap::new(),
    };
    let engine = EngineBuilder::new()
        .add_rule_group(vec![a_to_b, b_to_c, c_to_d])
        .build();
    (meta, engine)
}

#[test]
fn no_cache() {
    let expr = Expr::Quad(
        Box::new(Expr::C),
        Box::new(Expr::C),
        Box::new(Expr::C),
        Box::new(Expr::C),
    );

    let (meta, mut engine) = setup_nocache();

    let (expr, meta) = engine.morph(expr, meta);

    assert_eq!(
        expr,
        Expr::Quad(
            Box::new(Expr::D),
            Box::new(Expr::D),
            Box::new(Expr::D),
            Box::new(Expr::D),
        )
    );

    assert_eq!(meta.attempts.keys().len(), 1);

    assert_eq!(meta.applied.keys().len(), 1);

    assert_eq!(meta.applied.get("c->d"), Some(4).as_ref());
}

#[test]
fn basic_caching() {
    let expr = Expr::Quad(
        Box::new(Expr::C),
        Box::new(Expr::C),
        Box::new(Expr::C),
        Box::new(Expr::C),
    );

    let (meta, mut engine) = setup();

    let (expr, meta) = engine.morph(expr, meta);

    assert_eq!(
        expr,
        Expr::Quad(
            Box::new(Expr::D),
            Box::new(Expr::D),
            Box::new(Expr::D),
            Box::new(Expr::D),
        )
    );

    assert_eq!(meta.attempts.keys().len(), 1);

    assert_eq!(meta.applied.keys().len(), 1);

    assert_eq!(meta.applied.get("c->d"), Some(1).as_ref());
}

#[test]
fn transitive_no_caching() {
    let expr = Expr::Triple(Box::new(Expr::A), Box::new(Expr::B), Box::new(Expr::C));

    let (meta, mut engine) = setup_nocache();
    let (expr, meta) = engine.morph(expr, meta);

    assert_eq!(
        expr,
        Expr::Triple(Box::new(Expr::D), Box::new(Expr::D), Box::new(Expr::D),)
    );

    assert_eq!(meta.applied.get("a->b"), Some(1).as_ref());
    assert_eq!(meta.applied.get("b->c"), Some(2).as_ref());
    assert_eq!(meta.applied.get("c->d"), Some(3).as_ref());
}

#[test]
fn transitive_caching() {
    let expr = Expr::Triple(Box::new(Expr::A), Box::new(Expr::B), Box::new(Expr::C));

    let (meta, mut engine) = setup();
    let (expr, meta) = engine.morph(expr, meta);

    assert_eq!(
        expr,
        Expr::Triple(Box::new(Expr::D), Box::new(Expr::D), Box::new(Expr::D),)
    );

    assert_eq!(meta.applied.get("a->b"), Some(1).as_ref());
    assert_eq!(meta.applied.get("b->c"), Some(1).as_ref());
    assert_eq!(meta.applied.get("c->d"), Some(1).as_ref());
}

// --- Ancestor caching tests ---

#[derive(Debug, Clone, PartialEq, Eq, Uniplate, Hash)]
#[uniplate()]
enum ArithExpr {
    Add(Box<ArithExpr>, Box<ArithExpr>),
    Mul(Box<ArithExpr>, Box<ArithExpr>),
    Pair(Box<ArithExpr>, Box<ArithExpr>),
    Val(i32),
}

fn val(n: i32) -> ArithExpr {
    ArithExpr::Val(n)
}
fn add(a: ArithExpr, b: ArithExpr) -> ArithExpr {
    ArithExpr::Add(Box::new(a), Box::new(b))
}
fn mul(a: ArithExpr, b: ArithExpr) -> ArithExpr {
    ArithExpr::Mul(Box::new(a), Box::new(b))
}
fn pair(a: ArithExpr, b: ArithExpr) -> ArithExpr {
    ArithExpr::Pair(Box::new(a), Box::new(b))
}

fn eval_add(_: &mut Commands<ArithExpr, ()>, expr: &ArithExpr, _: &()) -> Option<ArithExpr> {
    if let ArithExpr::Add(a, b) = expr {
        if let (ArithExpr::Val(x), ArithExpr::Val(y)) = (a.as_ref(), b.as_ref()) {
            return Some(ArithExpr::Val(x + y));
        }
    }
    None
}

fn eval_mul(_: &mut Commands<ArithExpr, ()>, expr: &ArithExpr, _: &()) -> Option<ArithExpr> {
    if let ArithExpr::Mul(a, b) = expr {
        if let (ArithExpr::Val(x), ArithExpr::Val(y)) = (a.as_ref(), b.as_ref()) {
            return Some(ArithExpr::Val(x * y));
        }
    }
    None
}

/// Ancestor caching: after the first copy of `(1+2)*(3+4)` fully reduces to `Val(21)`,
/// the cache maps `(1+2)*(3+4) → Val(21)`. The second identical copy gets a
/// single cache hit at the top level — one powerful hit instead of traversing the subtree.
#[test]
fn ancestor_caching() {
    static HITS: AtomicU64 = AtomicU64::new(0);
    static MISSES: AtomicU64 = AtomicU64::new(0);

    fn on_hit(_: &ArithExpr, _: &mut ()) {
        HITS.fetch_add(1, Ordering::Relaxed);
    }
    fn on_miss(_: &ArithExpr, _: &mut ()) {
        MISSES.fetch_add(1, Ordering::Relaxed);
    }

    // Pair( (1+2)*(3+4), (1+2)*(3+4) )
    let subtree = mul(add(val(1), val(2)), add(val(3), val(4)));
    let tree = pair(subtree.clone(), subtree);

    HITS.store(0, Ordering::Relaxed);
    MISSES.store(0, Ordering::Relaxed);

    let mut engine = EngineBuilder::new()
        .add_rule_group(rule_fns![eval_add, eval_mul])
        .add_cacher(HashMapCache::<_, StdHashKey>::new())
        .add_on_cache_hit(on_hit)
        .add_on_cache_miss(on_miss)
        .build();

    let (result, _) = engine.morph(tree, ());
    assert_eq!(result, pair(val(21), val(21)));

    let hits = HITS.load(Ordering::Relaxed);
    assert!(hits > 0, "Expected at least one cache hit from ancestor caching");
}

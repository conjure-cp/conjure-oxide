use std::{collections::HashMap, ops::DerefMut};

use tree_morph::{cache::HashMapCache, prelude::*};
use uniplate::Uniplate;

#[derive(Debug, Clone, PartialEq, Eq, Uniplate, Hash)]
#[uniplate()]
enum Expr {
    Double(Box<Expr>, Box<Expr>),
    Triple(Box<Expr>, Box<Expr>, Box<Expr>),
    Quad(Box<Expr>, Box<Expr>, Box<Expr>, Box<Expr>),
    A,
    B,
    C,
    D,
    N,
}

#[derive(Debug)]
struct Meta {
    attempts: HashMap<String, usize>,
    applied: HashMap<String, usize>,
}

fn a_to_b(cmd: &mut Commands<Expr, Meta>, expr: &Expr, meta: &Meta) -> Option<Expr> {
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

fn b_to_c(cmd: &mut Commands<Expr, Meta>, expr: &Expr, meta: &Meta) -> Option<Expr> {
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

fn c_to_d(cmd: &mut Commands<Expr, Meta>, expr: &Expr, meta: &Meta) -> Option<Expr> {
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

#[test]
fn basic_caching() {
    let expr = Expr::Quad(
        Box::new(Expr::C),
        Box::new(Expr::C),
        Box::new(Expr::C),
        Box::new(Expr::C),
    );

    let meta = Meta {
        applied: HashMap::new(),
        attempts: HashMap::new(),
    };

    let engine = EngineBuilder::new()
        .add_rule_group(vec![a_to_b, b_to_c, c_to_d])
        .add_cacher(HashMapCache::new())
        .build();

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

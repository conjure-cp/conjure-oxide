//! Here we test rule groups with differing priorities.
//! Rules in a higher-index group will be applied first, even if they apply to lower nodes in the tree.

use tree_morph::prelude::*;
use uniplate::Uniplate;

/// A simple language of two literals and a wrapper
#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate()]
enum Expr {
    A,               // a
    B,               // b
    Wrap(Box<Expr>), // [E]
}

/// [a] ~> a
fn rule_unwrap_a(_: &mut Commands<Expr, ()>, expr: &Expr, _: &()) -> Option<Expr> {
    if let Expr::Wrap(inner) = expr
        && let Expr::A = **inner
    {
        return Some(Expr::A);
    }
    None
}

/// a ~> b
fn rule_a_to_b(_: &mut Commands<Expr, ()>, expr: &Expr, _: &()) -> Option<Expr> {
    if let Expr::A = expr {
        return Some(Expr::B);
    }
    None
}

#[test]
fn same_group() {
    // If the rules are in the same group, unwrap_a will apply higher in the tree

    // [a]
    let expr = Expr::Wrap(Box::new(Expr::A));

    let engine = EngineBuilder::new()
        .add_rule(rule_unwrap_a as RuleFn<_, _>)
        .add_rule(rule_a_to_b as RuleFn<_, _>)
        .build();
    let (result, _) = engine.morph(expr, ());

    // [a] ~> a ~> b
    assert_eq!(result, Expr::B);
}

#[test]
fn a_to_b_first() {
    // a_to_b is in a higher group than unwrap_a, so it will be applied first to the lower expression

    // [a]
    let expr = Expr::Wrap(Box::new(Expr::A));

    let engine = EngineBuilder::new()
        .add_rule(rule_a_to_b as RuleFn<_, _>)
        .add_rule(rule_unwrap_a as RuleFn<_, _>)
        .build();
    let (result, _) = engine.morph(expr, ());

    // [a] ~> [b]
    assert_eq!(result, Expr::Wrap(Box::new(Expr::B)));
}

//! Here we test the `reduce_with_rule_groups` function.
//! Each rule group is applied to the whole tree as with `reduce_with_rules`, before the next group is tried.
//! Every time a change is made, the algorithm starts again with the first group.
//!
//! This lets us make powerful "evaluation" rules which greedily reduce the tree as much as possible, before other
//! "rewriting" rules are applied.

use tree_morph::{helpers::select_first, *};
use uniplate::derive::Uniplate;

/// A simple language of two literals and a wrapper
#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate()]
enum Expr {
    A,               // a
    B,               // b
    Wrap(Box<Expr>), // [E]
}

/// Rule container: holds a primitive function and implements the Rule trait
struct Rl(fn(&Expr) -> Option<Expr>);

impl Rule<Expr, ()> for Rl {
    fn apply(&self, _: &mut Commands<Expr, ()>, expr: &Expr, _: &()) -> Option<Expr> {
        self.0(expr)
    }
}

mod rules {
    use super::*;

    /// [a] ~> a
    pub fn unwrap_a(expr: &Expr) -> Option<Expr> {
        if let Expr::Wrap(inner) = expr {
            if let Expr::A = **inner {
                return Some(Expr::A);
            }
        }
        None
    }

    /// a ~> b
    pub fn a_to_b(expr: &Expr) -> Option<Expr> {
        if let Expr::A = expr {
            return Some(Expr::B);
        }
        None
    }
}

#[test]
fn test_same_group() {
    // If the rules are in the same group, unwrap_a will apply higher in the tree

    // [a]
    let expr = Expr::Wrap(Box::new(Expr::A));

    let (expr, _) = reduce_with_rule_groups(
        &[&[Rl(rules::unwrap_a), Rl(rules::a_to_b)]],
        select_first,
        expr,
        (),
    );

    // [a] ~> a ~> b
    assert_eq!(expr, Expr::B);
}

#[test]
fn test_a_to_b_first() {
    // a_to_b is in a higher group than unwrap_a, so it will be applied first to the lower expression

    // [a]
    let expr = Expr::Wrap(Box::new(Expr::A));

    let (expr, _) = reduce_with_rule_groups(
        &[&[Rl(rules::a_to_b)], &[Rl(rules::unwrap_a)]],
        select_first,
        expr,
        (),
    );

    // [a] ~> [b]
    assert_eq!(expr, Expr::Wrap(Box::new(Expr::B)));
}

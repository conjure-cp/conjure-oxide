//! Here we implement a simple lambda calculus interpreter.
//! The beta-reduction rule has the side effect of increasing a counter in the metadata.

use std::collections::HashSet;
use tree_morph::prelude::*;
use uniplate::Uniplate;

#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate()]
enum Expr {
    Abs(u32, Box<Expr>),
    App(Box<Expr>, Box<Expr>),
    Var(u32),
}

impl Expr {
    fn free_vars(&self) -> HashSet<u32> {
        match self {
            Expr::Var(x) => {
                let mut set = HashSet::new();
                set.insert(*x);
                set
            }
            Expr::App(m, n) => {
                let mut set = m.free_vars();
                set.extend(n.free_vars());
                set
            }
            Expr::Abs(x, m) => {
                let mut set = m.free_vars();
                set.remove(x);
                set
            }
        }
    }

    fn vars(&self) -> HashSet<u32> {
        match self {
            Expr::Var(x) => {
                let mut set = HashSet::new();
                set.insert(*x);
                set
            }
            Expr::App(m, n) => {
                let mut set = m.vars();
                set.extend(n.vars());
                set
            }
            Expr::Abs(x, m) => {
                let mut set = m.vars();
                set.insert(*x);
                set
            }
        }
    }
}

// Create a fresh unused variable
fn fresh(vars: &HashSet<u32>) -> u32 {
    *vars.iter().max().unwrap_or(&0) + 1
}

// Capture-avoiding substitution
fn subst(expr: &Expr, x: u32, f: &Expr) -> Expr {
    if !expr.free_vars().contains(&x) {
        return expr.clone(); // x is bound or unused
    }

    match expr {
        Expr::Var(y) => {
            if *y == x {
                f.clone()
            } else {
                Expr::Var(*y)
            }
        }
        Expr::App(m, n) => Expr::App(Box::new(subst(m, x, f)), Box::new(subst(n, x, f))),
        Expr::Abs(y, g) => {
            if *y == x {
                Expr::Abs(*y, g.clone()) // binding occurrence
            } else if !f.free_vars().contains(y) {
                Expr::Abs(*y, Box::new(subst(g, x, f))) // substitution is safe
            } else {
                let z = fresh(&f.vars().union(&f.vars()).copied().collect());
                let new_g = subst(g, *y, &Expr::Var(z));
                Expr::Abs(z, Box::new(subst(&new_g, x, f))) // capture avoidance
            }
        }
    }
}

// Beta-reduction
fn beta_reduce(expr: &Expr) -> Option<Expr> {
    if let Expr::App(m, n) = expr {
        if let Expr::Abs(x, g) = m.as_ref() {
            return Some(subst(g, *x, n));
        }
    }
    None
}

fn transform_beta_reduce(cmd: &mut Commands<Expr, u32>, expr: &Expr, _: &u32) -> Option<Expr> {
    let retval = beta_reduce(expr);
    if retval.is_some() {
        cmd.mut_meta(Box::new(|m: &mut u32| *m += 1));
    }
    retval
}

#[test]
fn simple_application() {
    // (\x. x) 1 -> 1
    let expr = Expr::App(
        Box::new(Expr::Abs(0, Box::new(Expr::Var(0)))),
        Box::new(Expr::Var(1)),
    );
    let (result, meta) = morph(vec![vec![transform_beta_reduce]], select_first, expr, 0);
    assert_eq!(result, Expr::Var(1));
    assert_eq!(meta, 1);
}

#[test]
fn nested_application() {
    // ((\x. x) (\y. y)) 1 -> 1
    let expr = Expr::App(
        Box::new(Expr::App(
            Box::new(Expr::Abs(0, Box::new(Expr::Var(0)))),
            Box::new(Expr::Abs(1, Box::new(Expr::Var(1)))),
        )),
        Box::new(Expr::Var(2)),
    );
    let (result, meta) = morph(vec![vec![transform_beta_reduce]], select_first, expr, 0);
    assert_eq!(result, Expr::Var(2));
    assert_eq!(meta, 2);
}

#[test]
fn capture_avoiding_substitution() {
    // (\x. (\y. x)) 1 -> (\y. 1)
    let expr = Expr::App(
        Box::new(Expr::Abs(0, Box::new(Expr::Abs(1, Box::new(Expr::Var(0)))))),
        Box::new(Expr::Var(1)),
    );
    let (result, meta) = morph(vec![vec![transform_beta_reduce]], select_first, expr, 0);
    assert_eq!(result, Expr::Abs(2, Box::new(Expr::Var(1))));
    assert_eq!(meta, 1);
}

#[test]
fn double_reduction() {
    // (\x. (\y. y)) 1 -> (\y. y)
    let expr = Expr::App(
        Box::new(Expr::Abs(0, Box::new(Expr::Abs(1, Box::new(Expr::Var(1)))))),
        Box::new(Expr::Var(2)),
    );
    let (result, meta) = morph(vec![vec![transform_beta_reduce]], select_first, expr, 0);
    assert_eq!(result, Expr::Abs(1, Box::new(Expr::Var(1))));
    assert_eq!(meta, 1);
}

#[test]
fn id() {
    // (\x. x) -> (\x. x)
    let expr = Expr::Abs(0, Box::new(Expr::Var(0)));
    let (expr, meta) = morph(vec![vec![transform_beta_reduce]], select_first, expr, 0);
    assert_eq!(expr, Expr::Abs(0, Box::new(Expr::Var(0))));
    assert_eq!(meta, 0);
}

#[test]
fn no_reduction() {
    // x -> x
    let expr = Expr::Var(1);
    let (result, meta) = morph(
        vec![vec![transform_beta_reduce]],
        select_first,
        expr.clone(),
        0,
    );
    assert_eq!(result, expr);
    assert_eq!(meta, 0);
}

#[test]
fn complex_expression() {
    // (((\x. (\y. x y)) (\z. z)) (\w. w)) -> (\y. (\w. w) y) -> (\w. w)
    let expr = Expr::App(
        Box::new(Expr::App(
            Box::new(Expr::Abs(
                0,
                Box::new(Expr::Abs(
                    1,
                    Box::new(Expr::App(Box::new(Expr::Var(0)), Box::new(Expr::Var(1)))),
                )),
            )),
            Box::new(Expr::Abs(2, Box::new(Expr::Var(2)))),
        )),
        Box::new(Expr::Abs(3, Box::new(Expr::Var(3)))),
    );
    let (result, meta) = morph(vec![vec![transform_beta_reduce]], select_first, expr, 0);
    assert_eq!(result, Expr::Abs(3, Box::new(Expr::Var(3))));
    assert_eq!(meta, 3);
}

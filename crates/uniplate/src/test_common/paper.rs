use proptest::prelude::*;

// Examples found in the Uniplate paper.

// Stmt and Expr to demonstrate and test multitype traversals.
#[derive(Eq, PartialEq, Clone, Debug)]
pub enum Stmt {
    Assign(String, Expr),
    Sequence(Vec<Stmt>),
    If(Expr, Box<Stmt>, Box<Stmt>),
    While(Expr, Box<Stmt>),
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub enum Expr {
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Val(i32),
    Var(String),
    Neg(Box<Expr>),
}

use self::Expr::*;
use self::Stmt::*;
pub fn proptest_exprs() -> impl Strategy<Value = Expr> {
    let leafs = prop_oneof![any::<i32>().prop_map(Val), any::<String>().prop_map(Var),];

    leafs.prop_recursive(10, 512, 2, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 2..2)
                .prop_map(|elems| Add(Box::new(elems[0].clone()), Box::new(elems[1].clone()))),
            prop::collection::vec(inner.clone(), 2..2)
                .prop_map(|elems| Sub(Box::new(elems[0].clone()), Box::new(elems[1].clone()))),
            prop::collection::vec(inner.clone(), 2..2)
                .prop_map(|elems| Mul(Box::new(elems[0].clone()), Box::new(elems[1].clone()))),
            prop::collection::vec(inner.clone(), 2..2)
                .prop_map(|elems| Div(Box::new(elems[0].clone()), Box::new(elems[1].clone()))),
            inner.prop_map(|inner| Neg(Box::new(inner.clone())))
        ]
    })
}

pub fn proptest_stmts() -> impl Strategy<Value = Stmt> {
    let leafs = prop_oneof![(".*", proptest_exprs()).prop_map(|(a, b)| Assign(a, b)),];

    leafs.prop_recursive(10, 512, 50, |inner| {
        prop_oneof![
            (proptest_exprs(), prop::collection::vec(inner.clone(), 2..2)).prop_map(
                move |(expr, stmts)| If(
                    expr,
                    Box::new(stmts[0].clone()),
                    Box::new(stmts[1].clone())
                )
            ),
            (proptest_exprs(), inner.clone())
                .prop_map(move |(expr, stmt)| While(expr, Box::new(stmt))),
            prop::collection::vec(inner.clone(), 0..50).prop_map(Sequence)
        ]
    })
}

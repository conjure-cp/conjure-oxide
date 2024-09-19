use crate::biplate::Uniplate;
use crate::Uniplate;
use prop::sample::SizeRange;
use proptest::prelude::*;

// Examples found in the Uniplate paper.

// Stmt and Expr to demonstrate and test multitype traversals.
#[derive(Eq, PartialEq, Clone, Debug, Uniplate)]
#[biplate(to=Expr)]
#[biplate(to=String,walk_into=[Expr])]
#[uniplate(walk_into=[Expr])]
pub enum Stmt {
    Assign(String, Expr),
    Sequence(Vec<Stmt>),
    If(Expr, Box<Stmt>, Box<Stmt>),
    While(Expr, Box<Stmt>),
}

#[derive(Eq, PartialEq, Clone, Debug, Uniplate)]
#[uniplate(walk_into=[Stmt])]
#[biplate(to=String)]
#[biplate(to=Stmt)]
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
    let leafs = prop_oneof![any::<i32>().prop_map(Val), "[a-z]*".prop_map(Var),];

    leafs.prop_recursive(8, 256, 10, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 2..3)
                .prop_map(|elems| Add(Box::new(elems[0].clone()), Box::new(elems[1].clone()))),
            prop::collection::vec(inner.clone(), 2..3)
                .prop_map(|elems| Sub(Box::new(elems[0].clone()), Box::new(elems[1].clone()))),
            prop::collection::vec(inner.clone(), 2..3)
                .prop_map(|elems| Mul(Box::new(elems[0].clone()), Box::new(elems[1].clone()))),
            prop::collection::vec(inner.clone(), 2..3)
                .prop_map(|elems| Div(Box::new(elems[0].clone()), Box::new(elems[1].clone()))),
            inner.prop_map(|inner| Neg(Box::new(inner.clone())))
        ]
    })
}

pub fn proptest_stmts() -> impl Strategy<Value = Stmt> {
    let leafs = prop_oneof![("[a-z]*", proptest_exprs()).prop_map(|(a, b)| Assign(a, b)),];

    leafs.prop_recursive(8, 256, 10, |inner| {
        prop_oneof![
            (proptest_exprs(), prop::collection::vec(inner.clone(), 2..4)).prop_map(
                move |(expr, stmts)| If(
                    expr,
                    Box::new(stmts[0].clone()),
                    Box::new(stmts[1].clone())
                )
            ),
            (proptest_exprs(), inner.clone())
                .prop_map(move |(expr, stmt)| While(expr, Box::new(stmt))),
            prop::collection::vec(inner.clone(), 0..10).prop_map(Sequence)
        ]
    })
}

proptest! {

#![proptest_config(ProptestConfig { max_shrink_iters:1000000, max_global_rejects:100000,cases:50,..Default::default()})]

#[test]
fn uniplate_children(ast in proptest_stmts(), new_children in proptest::collection::vec(proptest_stmts(),1..=10)) {
    let original_children = ast.children();
    prop_assume!(original_children.len() == new_children.len());

    let mut ast = ast.with_children(new_children.clone().into());

    prop_assert_eq!(im::Vector::<Stmt>::from(new_children),ast.children());

    ast = ast.with_children(original_children.clone());
    prop_assert_eq!(original_children,ast.children());

}

}

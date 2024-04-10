// Expr and Stmt from the paper, manually derived.

use std::iter::zip;

use im::vector;
//use uniplate::test_common::paper::*;
use uniplate::biplate::*;

use self::Expr::*;

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

impl Uniplate for Expr {
    fn uniplate(&self) -> (Tree<Self>, Box<dyn Fn(Tree<Self>) -> Self>) {
        match self.clone() {
            Add(f0, f1) => {
                // Field 0 - Box<Expr>
                let (f0_tree, f0_ctx) = <Expr as Biplate<Expr>>::biplate(&*f0);

                // Field 1 - Box<Expr>
                let (f1_tree, f1_ctx) = <Expr as Biplate<Expr>>::biplate(&*f1);

                let tree = Many(vector![f0_tree, f1_tree]);
                let ctx = Box::new(move |new_tree| {
                    let Many(ts) = new_tree else { panic!() };
                    assert_eq!(ts.len(), 2);
                    Add(
                        Box::new(f0_ctx(ts[0].clone())),
                        Box::new(f1_ctx(ts[1].clone())),
                    )
                });

                (tree, ctx)
            }
            Sub(f0, f1) => {
                // Field 0 - Box<Expr>
                let (f0_tree, f0_ctx) = <Expr as Biplate<Expr>>::biplate(&*f0);

                // Field 1 - Box<Expr>
                let (f1_tree, f1_ctx) = <Expr as Biplate<Expr>>::biplate(&*f1);

                let tree = Many(vector![f0_tree, f1_tree]);
                let ctx = Box::new(move |new_tree| {
                    let Many(ts) = new_tree else { panic!() };
                    assert_eq!(ts.len(), 2);
                    Add(
                        Box::new(f0_ctx(ts[0].clone())),
                        Box::new(f1_ctx(ts[1].clone())),
                    )
                });

                (tree, ctx)
            }
            Mul(f0, f1) => {
                // Field 0 - Box<Expr>
                let (f0_tree, f0_ctx) = <Expr as Biplate<Expr>>::biplate(&*f0);

                // Field 1 - Box<Expr>
                let (f1_tree, f1_ctx) = <Expr as Biplate<Expr>>::biplate(&*f1);

                let tree = Many(vector![f0_tree, f1_tree]);
                let ctx = Box::new(move |new_tree| {
                    let Many(ts) = new_tree else { panic!() };
                    assert_eq!(ts.len(), 2);
                    Add(
                        Box::new(f0_ctx(ts[0].clone())),
                        Box::new(f1_ctx(ts[1].clone())),
                    )
                });

                (tree, ctx)
            }
            Div(f0, f1) => {
                // Field 0 - Box<Expr>
                let (f0_tree, f0_ctx) = <Expr as Biplate<Expr>>::biplate(&*f0);

                // Field 1 - Box<Expr>
                let (f1_tree, f1_ctx) = <Expr as Biplate<Expr>>::biplate(&*f1);

                let tree = Many(vector![f0_tree, f1_tree]);
                let ctx = Box::new(move |new_tree| {
                    let Many(ts) = new_tree else { panic!() };
                    assert_eq!(ts.len(), 2);
                    Add(
                        Box::new(f0_ctx(ts[0].clone())),
                        Box::new(f1_ctx(ts[1].clone())),
                    )
                });

                (tree, ctx)
            }

            Val(f0) => (
                Zero,
                Box::new(move |x| {
                    let Zero = x else { panic!() };
                    Val(f0)
                }),
            ),
            Var(f0) => (
                Zero,
                Box::new(move |x| {
                    let Zero = x else { panic!() };
                    Var(f0.clone())
                }),
            ),

            Neg(f0) => (
                Zero,
                Box::new(move |x| {
                    let Zero = x else { panic!() };
                    Neg(f0.clone())
                }),
            ),
        }
    }
}

impl Biplate<Stmt> for Expr {
    fn biplate(&self) -> (Tree<Stmt>, Box<dyn Fn(Tree<Stmt>) -> Expr>) {
        // Optimisation - in derivation, build index of types that lead to eachother.
        // Walk this graph to generate all "reachable types from expr"
        //
        // Stmt is not reachable so just return 0.
        //
        // Paper does this with the combinators!
        //
        // We may also need this to know what Biplates to derive!
        let expr = self.clone();
        (
            Zero,
            Box::new(move |stmt| {
                let Zero = stmt else { panic!() };
                expr.clone()
            }),
        )
    }
}

//this is the most interesting example!
#[allow(clippy::type_complexity)]
impl Biplate<Expr> for Stmt {
    fn biplate(&self) -> (Tree<Expr>, Box<dyn Fn(Tree<Expr>) -> Stmt>) {
        match self.clone() {
            Assign(f0, f1) => {
                // Field 0 - non recursive (String)
                let (f0_tree, f0_ctx) = (
                    Tree::<Expr>::Zero,
                    Box::new(move |_: Tree<Expr>| f0.clone()),
                );

                //field 1 - Expr - target type
                let (f1_tree, f1_ctx) = <Expr as Biplate<Expr>>::biplate(&f1);

                let tree = Tree::<Expr>::Many(vector![f0_tree, f1_tree]);

                let ctx = Box::new(move |new_tree| {
                    let Many(ts) = new_tree else { panic!() };
                    assert_eq!(ts.len(), 2);
                    Assign(f0_ctx(ts[0].clone()), f1_ctx(ts[1].clone()))
                });

                (tree, ctx)
            }
            Sequence(f0) => {
                // Field 0 - Vec<Stmt>

                // Get trees and contexts for each element.
                let (f0_elems, f0_ctxs): (Vec<Tree<Expr>>, Vec<Box<dyn Fn(Tree<Expr>) -> Stmt>>) =
                    f0.into_iter()
                        .map(|stmt| <Stmt as Biplate<Expr>>::biplate(&stmt))
                        .unzip();

                let f0_tree = Many(f0_elems.into());
                let f0_ctx: Box<dyn Fn(Tree<Expr>) -> Vec<Stmt>> = Box::new(move |new_tree| {
                    let Many(elem_ts) = new_tree else {
                        panic!();
                    };

                    zip(&f0_ctxs, elem_ts).map(|(ctx, t)| (**ctx)(t)).collect()
                });

                let tree = Many(vector![f0_tree]);
                let ctx = Box::new(move |new_tree| {
                    let Many(ts) = new_tree else {
                        panic!();
                    };
                    assert_eq!(ts.len(), 1);
                    Sequence(f0_ctx(ts[0].clone()))
                });

                (tree, ctx)
            }

            If(f0, f1, f2) => {
                // Field 0 - Expr
                let (f0_tree, f0_ctx) = <Expr as Biplate<Expr>>::biplate(&f0);

                // Field 1 - Box::(stmt)
                let (f1_tree, f1_ctx) = <Stmt as Biplate<Expr>>::biplate(&*f1);

                //Field 2 - Box::(Stmt)
                let (f2_tree, f2_ctx) = <Stmt as Biplate<Expr>>::biplate(&*f2);

                let tree = Many(vector![f0_tree, f1_tree, f2_tree]);
                let ctx = Box::new(move |new_tree| {
                    let Many(ts) = new_tree else { panic!() };
                    assert_eq!(ts.len(), 3);
                    If(
                        f0_ctx(ts[0].clone()),
                        Box::new(f1_ctx(ts[1].clone())),
                        Box::new(f2_ctx(ts[2].clone())),
                    )
                });

                (tree, ctx)
            }
            While(f0, f1) => {
                // Field 0 - Expr
                let (f0_tree, f0_ctx) = <Expr as Biplate<Expr>>::biplate(&f0);

                //Field 1 - Box::(Stmt)
                let (f1_tree, f1_ctx) = <Stmt as Biplate<Expr>>::biplate(&*f1);

                let tree = Many(vector![f0_tree, f1_tree]);
                let ctx = Box::new(move |new_tree| {
                    let Many(ts) = new_tree else { panic!() };
                    assert_eq!(ts.len(), 2);
                    While(f0_ctx(ts[0].clone()), Box::new(f1_ctx(ts[1].clone())))
                });

                (tree, ctx)
            }
        }
    }
}

impl Biplate<Expr> for Expr {
    fn biplate(&self) -> (Tree<Expr>, Box<dyn Fn(Tree<Expr>) -> Self>) {
        (
            One(self.clone()),
            Box::new(|t| {
                let One(stmt) = t else { panic!() };
                stmt
            }),
        )
    }
}

impl Biplate<Stmt> for Stmt {
    fn biplate(&self) -> (Tree<Stmt>, Box<dyn Fn(Tree<Stmt>) -> Self>) {
        (
            One(self.clone()),
            Box::new(|t| {
                let One(stmt) = t else { panic!() };
                stmt
            }),
        )
    }
}

use Stmt::*;
use Tree::*;

impl Uniplate for Stmt {
    fn uniplate(&self) -> (Tree<Stmt>, Box<dyn Fn(Tree<Stmt>) -> Stmt>) {
        match self.clone() {
            Assign(s, expr) => {
                // Field 0 - non recursive (String)
                let (f0_tree, f0_ctx) =
                    (Tree::<Stmt>::Zero, Box::new(move |_: Tree<Stmt>| s.clone()));

                // Field 1- ADT (Expr)
                let (f1_tree, f1_ctx) = <Expr as Biplate<Stmt>>::biplate(&expr);

                // we know there is no path Expr -> Stmt, so we could just inline the zero
                // defintion (see Biplate<Stmt> for Expr comments)
                // let (f1_tree,f1_ctx) (Zero, Box::new(move |stmt| {let Zero = stmt else {panic!()}; f1.clone()}));

                let tree = Many(vector![f0_tree, f1_tree]);
                let ctx = Box::new(move |new_tree| {
                    let Many(ts) = new_tree else { panic!() };
                    assert_eq!(ts.len(), 2);
                    Assign(f0_ctx(ts[0].clone()), f1_ctx(ts[1].clone()))
                });

                (tree, ctx)
            }
            Sequence(f0) => {
                // Field 0 - Vec<Stmt>
                // Special case for iterables / lists?

                // Get trees and contexts for each element.
                let (f0_elems, f0_ctxs): (Vec<Tree<Stmt>>, Vec<Box<dyn Fn(Tree<Stmt>) -> Stmt>>) =
                    f0.into_iter()
                        .map(|stmt| <Stmt as Biplate<Stmt>>::biplate(&stmt))
                        .unzip();

                let f0_tree = Many(f0_elems.into());
                let f0_ctx: Box<dyn Fn(Tree<Stmt>) -> Vec<Stmt>> = Box::new(move |new_tree| {
                    let Many(elem_ts) = new_tree else {
                        panic!();
                    };

                    zip(&f0_ctxs, elem_ts).map(|(ctx, t)| (**ctx)(t)).collect()
                });

                let tree = Many(vector![f0_tree]);
                let ctx = Box::new(move |new_tree| {
                    let Many(ts) = new_tree else {
                        panic!();
                    };
                    assert_eq!(ts.len(), 1);
                    Sequence(f0_ctx(ts[0].clone()))
                });

                (tree, ctx)
            }
            If(f0, f1, f2) => {
                // Field 0 - Expr
                let (f0_tree, f0_ctx) = <Expr as Biplate<Stmt>>::biplate(&f0);

                //field 1 - box::(stmt)
                // Treat T, Box<T> as a special case as defining Uniplate and Biplate for Box<T> is
                // a lot of moving things from stack to heap and back for no reason.
                let (f1_tree, f1_ctx) = <Stmt as Biplate<Stmt>>::biplate(&*f1);

                //Field 2 - Box::(Stmt)
                let (f2_tree, f2_ctx) = <Stmt as Biplate<Stmt>>::biplate(&*f2);

                let tree = Many(vector![f0_tree, f1_tree, f2_tree]);
                let ctx = Box::new(move |new_tree| {
                    let Many(ts) = new_tree else { panic!() };
                    assert_eq!(ts.len(), 3);
                    If(
                        f0_ctx(ts[0].clone()),
                        Box::new(f1_ctx(ts[1].clone())),
                        Box::new(f2_ctx(ts[2].clone())),
                    )
                });

                (tree, ctx)
            }
            While(f0, f1) => {
                // Field 0 - Expr
                let (f0_tree, f0_ctx) = <Expr as Biplate<Stmt>>::biplate(&f0);

                //Field 1 - Box::(Stmt)
                let (f1_tree, f1_ctx) = <Stmt as Biplate<Stmt>>::biplate(&*f1);

                let tree = Many(vector![f0_tree, f1_tree]);
                let ctx = Box::new(move |new_tree| {
                    let Many(ts) = new_tree else { panic!() };
                    assert_eq!(ts.len(), 2);
                    While(f0_ctx(ts[0].clone()), Box::new(f1_ctx(ts[1].clone())))
                });

                (tree, ctx)
            }
        }
    }
}

#[test]
fn children_bi_multitype() {
    let my_stmt = Sequence(vec![
        While(
            Val(0),
            Box::new(Assign(
                "x".to_owned(),
                Add(Box::new(Var("x".to_owned())), Box::new(Val(10))),
            )),
        ),
        If(
            Var("x".to_string()),
            Box::new(Assign(
                "x".to_string(),
                Add(Box::new(Var("x".to_owned())), Box::new(Val(10))),
            )),
            Box::new(Sequence(vec![])),
        ),
    ]);

    let expected_expr_children = 4;

    let children = <Stmt as Biplate<Expr>>::children_bi(&my_stmt);

    assert_eq!(expected_expr_children, children.len());

    println!("{:?}", children);
    let Val(_) = children[0] else { panic!() };
    let Add(_, _) = children[1] else { panic!() };
    let Var(_) = children[2] else { panic!() };
    let Add(_, _) = children[3] else { panic!() };
}

#[test]
fn universe_bi_multitype() {
    let my_stmt = Sequence(vec![
        While(
            Val(0),
            Box::new(Assign(
                "x".to_owned(),
                Add(Box::new(Var("x".to_owned())), Box::new(Val(10))),
            )),
        ),
        If(
            Var("x".to_string()),
            Box::new(Assign(
                "x".to_string(),
                Add(Box::new(Var("x".to_owned())), Box::new(Val(10))),
            )),
            Box::new(Sequence(vec![])),
        ),
    ]);

    let expected_expr_universe = 8;

    let children = <Stmt as Biplate<Expr>>::universe_bi(&my_stmt);

    assert_eq!(expected_expr_universe, children.len());

    println!("{:?}", children);
    let Val(_) = children[0] else { panic!() };
    let Add(_, _) = children[1] else { panic!() };
    let Var(_) = children[2] else { panic!() };
    let Val(_) = children[3] else { panic!() };
    let Var(_) = children[4] else { panic!() };
    let Add(_, _) = children[5] else { panic!() };
    let Var(_) = children[6] else { panic!() };
    let Val(_) = children[7] else { panic!() };
}

use tree_morph::prelude::*;
use uniplate::Uniplate;

#[derive(Debug, PartialEq, Eq, Clone, Uniplate)]
enum Expr {
    Lit(i32),
    Add(Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
}

struct Meta {
    on_enters: i32,
    on_exits: i32,
    stack: Vec<Expr>,
}

impl Meta {
    fn new() -> Self {
        Meta {
            on_enters: 0,
            on_exits: 0,
            stack: Vec::new(),
        }
    }
}

fn do_nothing(_: &mut Commands<Expr, Meta>, _: &Expr, _: &Meta) -> Option<Expr> {
    None
}

#[test]
fn explore_once_events_called_correct_amount() {
    let expr = Expr::Add(Box::new(Expr::Lit(1)), Box::new(Expr::Lit(1)));

    let engine = EngineBuilder::new()
        .add_rule(do_nothing)
        .add_on_enter(|_, meta| meta.on_enters += 1)
        .add_on_exit(|_, meta| meta.on_exits += 1)
        .build();
    let (_, new_meta) = engine.morph(expr, Meta::new());

    // Starting on the root node counts as entering its subtree
    assert_eq!(new_meta.on_enters, 3);
    assert_eq!(new_meta.on_exits, 2);
}

#[test]
fn explore_nested_events_called_correct_amount() {
    let expr = Expr::Neg(Box::new(Expr::Neg(Box::new(Expr::Neg(Box::new(
        Expr::Lit(1),
    ))))));

    let engine = EngineBuilder::new()
        .add_rule(do_nothing)
        .add_on_enter(|_, meta| meta.on_enters += 1)
        .add_on_exit(|_, meta| meta.on_exits += 1)
        .build();
    let (_, new_meta) = engine.morph(expr, Meta::new());

    assert_eq!(new_meta.on_enters, 4);
    assert_eq!(new_meta.on_exits, 3);
}

#[test]
fn correct_enter_order_called_at_leaf() {
    let expr = Expr::Add(
        Box::new(Expr::Neg(Box::new(Expr::Lit(42)))),
        Box::new(Expr::Lit(0)),
    );

    let engine = EngineBuilder::new()
        .add_rule(|_: &mut Commands<_, _>, expr: &Expr, meta: &Meta| {
            if let Expr::Lit(42) = expr {
                // We are at the first leaf, the path from the root should be in the stack
                assert!(matches!(meta.stack[0], Expr::Add(_, _)));
                assert!(matches!(meta.stack[1], Expr::Neg(_)));
                assert!(matches!(meta.stack[2], Expr::Lit(_)));
            }
            None
        })
        .add_on_exit(|_, meta| {
            meta.stack.pop().expect("empty stack popped");
        })
        .add_on_enter(|subtree, meta| meta.stack.push(subtree.clone()))
        .build();
    let (_, new_meta) = engine.morph(expr, Meta::new());

    // Only the root should be on the stack
    assert_eq!(new_meta.stack.len(), 1);
    assert!(matches!(new_meta.stack[0], Expr::Add(_, _)));
}

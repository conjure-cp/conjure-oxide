use tree_morph::prelude::*;
use uniplate::Uniplate;

#[derive(Debug, PartialEq, Eq, Clone, Uniplate)]
enum Expr {
    Lit(i32),
    Add(Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
}

#[derive(Default)]
struct Meta {
    before_downs: i32,
    after_downs: i32,
    before_rights: i32,
    after_rights: i32,
    before_ups: i32,
    after_ups: i32,
    stack: Vec<Expr>,
}

impl Meta {
    fn new() -> Self {
        Default::default()
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
        .add_before_up(|_, meta| meta.before_ups += 1)
        .add_after_up(|_, meta| meta.after_ups += 1)
        .add_before_down(|node, meta| {
            meta.before_downs += 1;
            println!("{node:?}")
        })
        .add_after_down(|_, meta| meta.after_downs += 1)
        .add_before_right(|_, meta| meta.before_rights += 1)
        .add_after_right(|_, meta| meta.after_rights += 1)
        .build();
    let (_, new_meta) = engine.morph(expr, Meta::new());

    // Moves down, right, up
    assert_eq!(new_meta.before_ups, 1);
    assert_eq!(new_meta.after_ups, 1);
    assert_eq!(new_meta.before_downs, 1);
    assert_eq!(new_meta.after_downs, 1);
    assert_eq!(new_meta.before_rights, 1);
    assert_eq!(new_meta.after_rights, 1);
}

#[test]
fn explore_nested_events_called_correct_amount() {
    let expr = Expr::Neg(Box::new(Expr::Neg(Box::new(Expr::Neg(Box::new(
        Expr::Lit(1),
    ))))));

    let engine = EngineBuilder::new()
        .add_rule(do_nothing)
        .add_before_up(|_, meta| meta.before_ups += 1)
        .add_after_up(|_, meta| meta.after_ups += 1)
        .add_before_down(|_, meta| meta.before_downs += 1)
        .add_after_down(|_, meta| meta.after_downs += 1)
        .add_before_right(|_, meta| meta.before_rights += 1)
        .add_after_right(|_, meta| meta.after_rights += 1)
        .build();
    let (_, new_meta) = engine.morph(expr, Meta::new());

    // Moves down, down, down, up, up, up
    assert_eq!(new_meta.before_ups, 3);
    assert_eq!(new_meta.after_ups, 3);
    assert_eq!(new_meta.before_downs, 3);
    assert_eq!(new_meta.after_downs, 3);
    assert_eq!(new_meta.before_rights, 0);
    assert_eq!(new_meta.after_rights, 0);
}

#[test]
fn correct_order_pushed_to_stack() {
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
            }
            None
        })
        .add_before_down(|subtree, meta| meta.stack.push(subtree.clone()))
        .add_after_up(|_, meta| {
            meta.stack.pop().expect("empty stack popped");
        })
        .build();
    let (_, new_meta) = engine.morph(expr, Meta::new());

    // After returning to the root, the stack should be empty.
    assert_eq!(new_meta.stack.len(), 0);
}

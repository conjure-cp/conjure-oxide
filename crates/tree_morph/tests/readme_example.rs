use tree_morph::prelude::*;
use uniplate::derive::Uniplate;

#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate()]
enum Expr {
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Sqr(Box<Expr>),
    Val(i32),
}

fn rule_eval_add(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
    if let Expr::Add(a, b) = subtree {
        if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
            cmds.mut_meta(|m| m.num_applications_addition += 1);
            cmds.transform(|t|)
            return Some(Expr::Val(a_v + b_v));
        }
    }
    None
}

fn rule_eval_mul(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
    if let Expr::Mul(a, b) = subtree {
        if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
            return Some(Expr::Val(a_v * b_v));
        }
    }
    None
}

fn rule_expand_sqr(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
    if let Expr::Sqr(expr) = subtree {
        return Some(Expr::Mul(Box::new(*expr.clone()), Box::new(*expr.clone())));
    }
    None
}

#[derive(Debug)]
struct Meta {
    num_applications_addition: i32,
}

#[test]
fn easy_test() {
    let my_expression = Expr::Sqr(Box::new(Expr::Add(
        Box::new(Expr::Val(1)),
        Box::new(Expr::Val(2)),
    )));

    let bruh = Meta {
        num_applications_addition: 0,
    };
    let (result, foo) = morph(
        vec![rule_fns![rule_eval_add, rule_eval_mul, rule_expand_sqr]],
        tree_morph::helpers::select_panic,
        my_expression.clone(),
        bruh,
    );
    assert_eq!(foo.num_applications_addition, 2);
}

#[test]
fn no_operations() {
    let my_expression = Expr::Sqr(Box::new(Expr::Add(
        Box::new(Expr::Val(1)),
        Box::new(Expr::Val(2)),
    )));

    let metadata = Meta {
        num_applications_addition: 0,
    };
    let (result, metaresult) = morph(
        vec![
            rule_fns![rule_eval_add, rule_eval_mul],
            rule_fns![rule_expand_sqr],
        ],
        tree_morph::helpers::select_panic,
        my_expression.clone(),
        metadata,
    );
    assert_eq!(metaresult.num_applications_addition, 1);
}

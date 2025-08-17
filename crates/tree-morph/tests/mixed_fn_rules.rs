use tree_morph::prelude::*;
use uniplate::Uniplate;

#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate()]
enum Expr {
    A,
    B,
    C,
    D,
}

fn rule_b_to_c(_: &mut Commands<Expr, ()>, expr: &Expr, _: &()) -> Option<Expr> {
    if let Expr::B = expr {
        return Some(Expr::C);
    }
    None
}

#[test]
fn closure_rules() {
    let expr = Expr::A;

    let (result, _) = morph(
        vec![
            vec![
                (|_, t, _| match t {
                    Expr::A => Some(Expr::B),
                    _ => None,
                }) as RuleFn<_, _>, // Same as macro expansion
            ],
            rule_fns![
                |_, t, _| match t {
                    Expr::C => Some(Expr::D),
                    _ => None,
                },
                rule_b_to_c,
            ],
        ],
        select_first,
        expr,
        (),
    );

    assert_eq!(result, Expr::D);
}
